/** Response shapes from the Baitler backend API. */

/** `GET /health` */
export interface HealthResponse {
  status: string;
  db: string;
}

/** `GET /version` */
export interface VersionResponse {
  name: string;
  version: string;
  git_sha: string | null;
}

/** The backend's standard JSON error envelope. */
export interface ApiErrorBody {
  error: {
    code: string;
    message: string;
  };
}
