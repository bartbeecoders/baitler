/** Document API shapes (mirror the backend DTOs). */

export interface DocumentSummary {
  id: string;
  title: string;
  version: number;
  tags: string[];
  updated_at: string;
}

export interface Document {
  id: string;
  title: string;
  body: string;
  version: number;
  tags: string[];
  created_at: string;
  updated_at: string;
}

export type ExportFormat = 'pdf' | 'docx' | 'html' | 'markdown';
export type ExportSource = 'html' | 'markdown';
