ALTER TABLE jobs ADD COLUMN selected_platforms TEXT NOT NULL DEFAULT '[]';

-- Backfill from existing explicit platform column only.
UPDATE jobs
SET selected_platforms = '["' || platform || '"]'
WHERE selected_platforms = '[]' AND platform IN ('linkedin', 'x');
