/** AI feature API shapes (mirror the backend DTOs). */

export interface ModelInfo {
  id: string;
  label: string;
  modalities: string[];
}

export interface Provider {
  id: string;
  label: string;
  requires_key: boolean;
  /** Whether this owner can use the provider (mock, or a key is stored). */
  configured: boolean;
  models: ModelInfo[];
}

export type ChatRole = 'user' | 'assistant';

export interface ChatMessage {
  role: ChatRole;
  content: string;
}
