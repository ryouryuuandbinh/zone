/** Vietnamese searches accept both accented and unaccented words. */
export function searchKey(value: string): string {
  return value
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .replace(/[đĐ]/g, 'd')
    .toLocaleLowerCase('vi')
    .trim();
}
export function matchesSearch(query: string, ...values: string[]): boolean {
  const words = searchKey(query).split(/\s+/).filter(Boolean);
  const haystack = searchKey(values.join(' '));
  return words.every((word) => haystack.includes(word));
}
