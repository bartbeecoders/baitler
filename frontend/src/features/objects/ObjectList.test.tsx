import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { ObjectList } from './ObjectList';

function setup(overrides: Partial<Parameters<typeof ObjectList>[0]> = {}) {
  const props = {
    noun: 'document',
    items: [
      { id: 'a', title: 'Alpha' },
      { id: 'b', title: 'Beta', badge: { label: 'Draft', variant: 'warning' as const } },
    ],
    isLoading: false,
    activeId: 'a',
    search: '',
    onSearch: vi.fn(),
    filters: [
      {
        key: 'tag',
        label: 'All tags',
        value: '',
        onChange: vi.fn(),
        options: [{ value: 'work', label: 'work' }],
      },
    ],
    onSelect: vi.fn(),
    onCreate: vi.fn(),
    onDelete: vi.fn(),
    onRefresh: vi.fn(),
    ...overrides,
  };
  render(<ObjectList {...props} />);
  return props;
}

describe('ObjectList', () => {
  it('renders the +/refresh/trash controls, search, filter, and items', () => {
    setup();
    expect(screen.getByLabelText('New document')).toBeInTheDocument();
    expect(screen.getByLabelText('Refresh document list')).toBeInTheDocument();
    expect(screen.getByLabelText('Search documents')).toBeInTheDocument();
    expect(screen.getByLabelText('All tags')).toBeInTheDocument();
    expect(screen.getByText('Alpha')).toBeInTheDocument();
    expect(screen.getByText('Beta')).toBeInTheDocument();
    expect(screen.getByText('Draft')).toBeInTheDocument();
  });

  it('calls onCreate, onRefresh, onSearch, and onSelect', () => {
    const props = setup();
    fireEvent.click(screen.getByLabelText('New document'));
    expect(props.onCreate).toHaveBeenCalled();
    fireEvent.click(screen.getByLabelText('Refresh document list'));
    expect(props.onRefresh).toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText('Search documents'), { target: { value: 'x' } });
    expect(props.onSearch).toHaveBeenCalledWith('x');
    fireEvent.click(screen.getByText('Beta'));
    expect(props.onSelect).toHaveBeenCalledWith('b');
  });

  it('requires a second click to confirm a delete of the active item', () => {
    const props = setup();
    const trash = screen.getByLabelText('Delete selected document');
    fireEvent.click(trash); // arms confirm
    expect(props.onDelete).not.toHaveBeenCalled();
    fireEvent.click(screen.getByLabelText('Confirm delete document'));
    expect(props.onDelete).toHaveBeenCalledWith('a');
  });

  it('disables the trash button when nothing is selected', () => {
    setup({ activeId: null });
    expect(screen.getByLabelText('Delete selected document')).toBeDisabled();
  });
});
