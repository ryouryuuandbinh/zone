import type { ReactNode } from 'react';
import { AlertCircle, Search, X } from 'lucide-react';

export function FeatureSearch({
  value,
  onChange,
  placeholder,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
}) {
  return (
    <label className="feature-search">
      <Search size={14} />
      <input
        value={value}
        maxLength={2000}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        aria-label={placeholder}
      />
      {value && (
        <button type="button" onClick={() => onChange('')} aria-label="Xóa tìm kiếm">
          <X size={12} />
        </button>
      )}
    </label>
  );
}
export function FeatureEmpty({
  icon,
  title,
  children,
}: {
  icon: ReactNode;
  title: string;
  children: ReactNode;
}) {
  return (
    <div className="feature-empty">
      <span>{icon}</span>
      <strong>{title}</strong>
      <p>{children}</p>
    </div>
  );
}
export function FeatureError({ message }: { message: string | null }) {
  return message ? (
    <p className="feature-error" role="alert">
      <AlertCircle size={12} />
      <span>{message}</span>
    </p>
  ) : null;
}
export function PreviewNotice() {
  return <p className="feature-preview">Mở ứng dụng Zone trên Windows để sử dụng tính năng này.</p>;
}
