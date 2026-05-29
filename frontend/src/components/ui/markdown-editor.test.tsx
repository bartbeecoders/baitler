import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { MarkdownEditor } from './markdown-editor';

describe('MarkdownEditor', () => {
  it('edits via the textarea and renders Markdown in preview', async () => {
    const onChange = vi.fn();
    render(<MarkdownEditor value={'# Heading'} onChange={onChange} />);

    // Write tab shows a textarea with the raw markdown.
    const textarea = screen.getByRole('textbox', { name: /markdown content/i });
    expect(textarea).toHaveValue('# Heading');
    await userEvent.type(textarea, '!');
    expect(onChange).toHaveBeenCalled();

    // Preview tab renders the markdown as a heading.
    await userEvent.click(screen.getByRole('button', { name: 'Preview' }));
    expect(screen.getByRole('heading', { name: 'Heading' })).toBeInTheDocument();
  });
});
