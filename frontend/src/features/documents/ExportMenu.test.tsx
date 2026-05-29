import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ExportMenu } from './ExportMenu';

describe('ExportMenu', () => {
  it('opens and lists the export formats', async () => {
    render(<ExportMenu content="<p>hi</p>" source="html" filename="doc" />);
    await userEvent.click(screen.getByRole('button', { name: /export/i }));
    expect(screen.getByText('PDF')).toBeInTheDocument();
    expect(screen.getByText('Word (.docx)')).toBeInTheDocument();
    expect(screen.getByText('HTML')).toBeInTheDocument();
    expect(screen.getByText('Markdown')).toBeInTheDocument();
  });
});
