-- 0004_ai — per-owner LLM provider API keys (encrypted at rest).
--
-- `ciphertext` is ChaCha20-Poly1305 (see src/crypto.rs); the plaintext key is
-- never stored or returned. One key per (owner, provider).

DEFINE TABLE IF NOT EXISTS provider_key SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS owner ON provider_key TYPE string;
DEFINE FIELD IF NOT EXISTS provider ON provider_key TYPE string;
DEFINE FIELD IF NOT EXISTS ciphertext ON provider_key TYPE string;
DEFINE FIELD IF NOT EXISTS created_at ON provider_key TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON provider_key TYPE datetime VALUE time::now();
DEFINE INDEX IF NOT EXISTS provider_key_owner ON provider_key FIELDS owner, provider UNIQUE;
