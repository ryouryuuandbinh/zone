interface ZoneMarkProps {
  small?: boolean;
}

export function ZoneMark({ small = false }: ZoneMarkProps) {
  return (
    <span className={`zone-mark${small ? ' zone-mark-small' : ''}`} aria-hidden="true">
      <span />
      <span />
    </span>
  );
}
