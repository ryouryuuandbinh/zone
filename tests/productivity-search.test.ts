import { describe, expect, it } from 'vitest';
import { matchesSearch, searchKey } from '../src/features/shared/search';

describe('Vietnamese productivity search', () => {
  it('finds accented notes from an unaccented query', () => {
    expect(matchesSearch('du an den', 'Dự án Đèn', 'Việc cần làm')).toBe(true);
    expect(searchKey('  ĐẶT LỊCH  ')).toBe('dat lich');
  });

  it('matches decomposed Unicode and searches across fields', () => {
    expect(matchesSearch('bao cao', 'Báo cáo'.normalize('NFD'))).toBe(true);
    expect(matchesSearch('khach hang', 'Cuộc họp với khách', 'Lịch chăm sóc hàng tuần')).toBe(true);
  });

  it('requires all query words and handles empty queries', () => {
    expect(matchesSearch('ghi chu quan trong', 'Ghi chú thường ngày')).toBe(false);
    expect(matchesSearch('   ', '')).toBe(true);
  });
});
