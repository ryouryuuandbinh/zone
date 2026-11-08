import { spawn } from 'node:child_process';
import { mkdtemp, realpath, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, dirname, basename } from 'node:path';
import process from 'node:process';
import { clearTimeout, setTimeout } from 'node:timers';
import react from '@vitejs/plugin-react';
import { createServer } from 'vite';

const browser = process.argv[2] ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const tempRoot = await realpath(tmpdir());
const profile = await mkdtemp(join(tempRoot, 'zone-editor-render-'));

const fixture = `<!doctype html>
<title>Zone screenshot renderer test</title>
<pre id="result">PENDING</pre>
<script>
const reportFailure = (message) => fetch('/zone-render-result', { method: 'POST', body: 'FAIL: ' + message });
window.addEventListener('error', (event) => reportFailure(event.message || 'Module load failed'));
window.addEventListener('unhandledrejection', (event) => reportFailure(event.reason?.message || String(event.reason)));
</script>
<script type="module">
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { ScreenshotPanel } from '/src/features/screenshot/ScreenshotPanel.tsx';
import { Editor } from '/src/features/screenshot/editor/Editor.tsx';
import { createDocument, createAnnotation, defaultAnnotationStyle, updateCrop, outputSize } from '/src/features/screenshot/editor/model.ts';
import { composeDocument } from '/src/features/screenshot/editor/render.ts';

const result = document.getElementById('result');
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const waitFor = (promise, message) => {
  let timer;
  const expired = new Promise((_, reject) => { timer = setTimeout(() => reject(new Error(message)), 7000); });
  return Promise.race([promise, expired]).finally(() => clearTimeout(timer));
};
const base = document.createElement('canvas');
base.width = 14;
base.height = 10;
const source = base.getContext('2d');
for (let y = 0; y < base.height; y++) {
  for (let x = 0; x < base.width; x++) {
    source.fillStyle = 'rgb(' + (x * 13) + ',' + (y * 19) + ',' + (x + y) + ')';
    source.fillRect(x, y, 1, 1);
  }
}

async function read(blob) {
  const bitmap = await createImageBitmap(blob);
  const copy = document.createElement('canvas');
  copy.width = bitmap.width;
  copy.height = bitmap.height;
  const context = copy.getContext('2d');
  context.drawImage(bitmap, 0, 0);
  bitmap.close();
  return { width: copy.width, height: copy.height, context };
}

async function checkEditorSession() {
  const screen = document.createElement('canvas');
  screen.width = 320;
  screen.height = 200;
  const context = screen.getContext('2d');
  context.fillStyle = '#334455';
  context.fillRect(0, 0, 320, 200);
  const source = URL.createObjectURL(await composeDocument(screen, createDocument(320, 200)));
  const mount = document.createElement('div');
  document.body.append(mount);
  const root = createRoot(mount);
  let signalReady;
  let signalFailure;
  let signalExport;
  let closed = 0;
  let copies = 0;
  const actions = [];
  const ready = new Promise((resolve, reject) => { signalReady = resolve; signalFailure = reject; });
  const nextFrame = () => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  const buttonFor = (name) => Array.from(mount.querySelectorAll('button')).find((item) =>
    item.getAttribute('aria-label') === name || item.textContent.trim() === name
  );
  const click = async (name) => {
    const button = buttonFor(name);
    assert(button && !button.disabled, 'Missing enabled editor action: ' + name);
    button.click();
    await nextFrame();
  };

  try {
    root.render(createElement(ScreenshotPanel));
    await nextFrame();
    const entryActions = Array.from(mount.querySelectorAll('button'));
    assert(entryActions.length === 1 && entryActions[0].textContent.trim() === 'Chụp màn hình', 'Screenshot panel has more than one capture entry');
    root.render(createElement(Editor, {
      frame: { id: 'shot-fixture', width: 320, height: 200 },
      sourceUrl: source,
      onReady: signalReady,
      onError: (message) => signalFailure(new Error(message)),
      onCancel: () => { closed++; },
      onExport: async (value) => {
        actions.push(value.action);
        signalExport?.(value);
        // Simulate an export that leaves the editor open, followed by a successful copy.
        return value.action === 'copy' && ++copies === 2;
      },
    }));
    await waitFor(ready, 'Editor did not finish loading the frozen image');
    await nextFrame();
    const stage = mount.querySelector('.shot-stage');
    const editor = mount.querySelector('.shot-editor');
    assert(stage && editor, 'Editor canvas was not mounted');
    const key = (value, ctrlKey = false, target = editor) =>
      target.dispatchEvent(new KeyboardEvent('keydown', { key: value, ctrlKey, bubbles: true, cancelable: true }));
    const exportWith = async (value, ctrlKey) => {
      const result = new Promise((resolve) => { signalExport = resolve; });
      key(value, ctrlKey);
      const output = await waitFor(result, 'Keyboard export did not run: ' + value);
      signalExport = undefined;
      await nextFrame();
      return output;
    };
    // Synthetic PointerEvents are not registered as active native pointers.
    stage.setPointerCapture = () => {};
    const drag = async (fromX, fromY, toX, toY) => {
      const rect = stage.getBoundingClientRect();
      const pointer = (kind, x, y) => stage.dispatchEvent(new PointerEvent(kind, {
        bubbles: true, cancelable: true, pointerId: 1, isPrimary: true, button: 0,
        clientX: rect.left + x * rect.width / 320,
        clientY: rect.top + y * rect.height / 200,
      }));
      pointer('pointerdown', fromX, fromY);
      pointer('pointermove', toX, toY);
      pointer('pointerup', toX, toY);
      await nextFrame();
    };

    assert(mount.textContent.includes('Kéo để chọn vùng · Esc để hủy'), 'Missing region selection hint');
    assert(!buttonFor('Hình chữ nhật') && !buttonFor('Sao chép'), 'Toolbar shown before selection');
    key('Enter');
    await nextFrame();
    assert(actions.length === 0, 'Editor exported without a selection');
    await drag(40, 40, 160, 140);
    const badge = () => mount.querySelector('.shot-region-size')?.textContent.trim();
    assert(badge() === '120 × 100 px', 'Region badge does not match the physical crop');
    assert(!mount.textContent.includes('Kéo để chọn vùng · Esc để hủy'), 'Selection hint still visible');
    assert(buttonFor('Sao chép') && buttonFor('Lưu ảnh'), 'Export actions missing from toolbar');
    assert(!buttonFor('Toàn màn hình') && !buttonFor('Xác nhận vùng'), 'An obsolete capture action is visible');

    const cropField = mount.querySelector('input[type="number"]');
    assert(cropField, 'Missing crop size input');
    key('c', true, cropField);
    await nextFrame();
    assert(actions.length === 0, 'Ctrl+C inside an input exported the image');
    await click('Hình chữ nhật');
    await drag(50, 50, 130, 110);
    key('z', true);
    await nextFrame();
    assert(badge() === '120 × 100 px', 'Undo changed the selected region');
    key('y', true);
    await nextFrame();

    const save = await exportWith('s', true);
    assert(save.action === 'save' && closed === 0 && badge() === '120 × 100 px', 'Cancelled save lost the selection');
    const copy = await exportWith('c', true);
    assert(copy.action === 'copy' && closed === 0, 'Ctrl+C did not use the copy action');
    const output = await exportWith('Enter');
    assert(output.action === 'copy' && output.width === 120 && output.height === 100, 'Wrong editor export');
    assert(closed === 1 && actions.join(',') === 'save,copy,copy', 'Successful copy did not finish exactly once');
    const decoded = await read(output.blob);
    assert(String(decoded.context.getImageData(10, 35, 1, 1).data) === '250,204,21,255', 'Selected export lost the annotation after redo');
  } finally {
    root.unmount();
    mount.remove();
    URL.revokeObjectURL(source);
  }
}

try {
  const scene = updateCrop(createDocument(14, 10), { x: 2, y: 1, width: 9, height: 7 });
  const clean = await read(await composeDocument(base, scene));
  assert(clean.width === 9 && clean.height === 7, 'Crop dimensions changed');
  const cropStart = clean.context.getImageData(0, 0, 1, 1).data;
  assert(String(cropStart) === '26,19,3,255', 'Crop starts at wrong source pixel');

  const vertical = createAnnotation('v', 'cutout', { x: 5, y: 0 }, { x: 7, y: 10 }, {
    ...defaultAnnotationStyle, cutoutDirection: 'vertical',
  });
  const horizontal = createAnnotation('h', 'cutout', { x: 0, y: 3 }, { x: 14, y: 4 }, {
    ...defaultAnnotationStyle, cutoutDirection: 'horizontal',
  });
  const stitched = { ...scene, annotations: [vertical, horizontal] };
  const pixels = await read(await composeDocument(base, stitched));
  const size = outputSize(stitched);
  assert(size.width === 7 && size.height === 6, 'Incorrect size after cutout');
  assert(pixels.width === size.width && pixels.height === size.height, 'Raster size differs from metadata');
  const xs = [2, 3, 4, 7, 8, 9, 10];
  const ys = [1, 2, 4, 5, 6, 7];
  for (const [y, sy] of ys.entries()) {
    for (const [x, sx] of xs.entries()) {
      const actual = pixels.context.getImageData(x, y, 1, 1).data;
      assert(String(actual) === String([sx * 13, sy * 19, sx + sy, 255]), 'Wrong stitched pixel ' + x + ',' + y);
    }
  }

  const rectangle = createAnnotation('rectangle', 'rect', { x: 4, y: 3 }, { x: 9, y: 7 }, {
    ...defaultAnnotationStyle, color: '#ff0000', fill: true, strokeWidth: 1,
  });
  const marked = { ...scene, annotations: [rectangle] };
  const image = await read(await composeDocument(base, marked));
  assert(String(image.context.getImageData(3, 3, 1, 1).data) === '255,0,0,255', 'Annotation absent in cropped PNG');

  const jpeg = await composeDocument(base, stitched, undefined, 'image/jpeg');
  assert(jpeg.type === 'image/jpeg', 'JPEG MIME mismatch');
  const decoded = await read(jpeg);
  assert(decoded.width === 7 && decoded.height === 6, 'JPEG dimensions changed');
  await checkEditorSession();
  result.textContent = 'PASS: region-first toolbar, crop badge, copy/save shortcuts, cancelled save, exact crop, Cut Out and PNG/JPEG';
} catch (error) {
  result.textContent = 'FAIL: ' + error.message;
}
await fetch('/zone-render-result', { method: 'POST', body: result.textContent });
</script>`;

let report;
const reported = new Promise((resolve) => {
  report = resolve;
});
const server = await createServer({
  configFile: false,
  server: { host: '127.0.0.1', port: 0, strictPort: true, hmr: false },
  plugins: [
    react(),
    {
      name: 'zone-render-check',
      configureServer(vite) {
        vite.middlewares.use('/zone-render-check', (_request, response, next) => {
          void vite.transformIndexHtml('/zone-render-check', fixture).then((html) => {
            response.setHeader('Content-Type', 'text/html; charset=utf-8');
            response.end(html);
          }, next);
        });
        vite.middlewares.use('/zone-render-result', (request, response) => {
          if (request.method !== 'POST') {
            response.statusCode = 405;
            response.end();
            return;
          }
          let outcome = '';
          request.setEncoding('utf8');
          request.on('data', (chunk) => {
            outcome = (outcome + chunk).slice(0, 1000);
          });
          request.on('end', () => {
            response.end();
            report(outcome);
          });
        });
      },
    },
  ],
});

try {
  await server.listen();
  const address = server.httpServer.address();
  if (!address || typeof address === 'string') throw new Error('Missing local test address');
  const url = `http://127.0.0.1:${address.port}/zone-render-check`;
  const child = spawn(
    browser,
    [
      '--headless',
      '--disable-background-networking',
      '--no-first-run',
      `--user-data-dir=${profile}`,
      url,
    ],
    { windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] },
  );
  let diagnostics = '';
  child.stderr.setEncoding('utf8').on('data', (chunk) => {
    diagnostics = (diagnostics + chunk).slice(-1800);
  });
  const stopped = new Promise((resolve) => child.once('close', resolve));
  const interrupted = new Promise((_, reject) => {
    child.once('error', reject);
    void stopped.then((code) => reject(new Error(diagnostics || `Browser exited with ${code}`)));
  });
  let timeout;
  const expired = new Promise((_, reject) => {
    timeout = setTimeout(() => reject(new Error(diagnostics || 'Browser check timed out')), 30000);
  });
  try {
    const outcome = await Promise.race([reported, interrupted, expired]);
    if (!outcome.startsWith('PASS:')) throw new Error(outcome);
    process.stdout.write(`${outcome}\n`);
  } finally {
    clearTimeout(timeout);
    if (child.exitCode === null) child.kill();
    await stopped;
  }
} finally {
  await server.close();
  const resolved = await realpath(profile);
  if (dirname(resolved) === tempRoot && basename(resolved).startsWith('zone-editor-render-')) {
    await rm(resolved, { recursive: true, force: true, maxRetries: 3, retryDelay: 150 });
  }
}
