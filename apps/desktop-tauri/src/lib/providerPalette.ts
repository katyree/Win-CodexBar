export const PROVIDER_PALETTE = [
  "#C3ABFF", "#F1A8CE", "#9DCFFF", "#98D9BE",
  "#EAD58F", "#F2B08C", "#AAB9E5", "#D8B9A0",
] as const;

const PROVIDER_COLORS: Record<string, string> = {
  codex: PROVIDER_PALETTE[0], jetbrains: PROVIDER_PALETTE[1],
  claude: PROVIDER_PALETTE[2], gemini: PROVIDER_PALETTE[3],
  copilot: PROVIDER_PALETTE[4], cursor: PROVIDER_PALETTE[5],
  kiro: PROVIDER_PALETTE[6], mistral: PROVIDER_PALETTE[7],
};

/** Identity-based assignment stays stable across reorder and refresh. */
export function providerColor(providerId: string): string {
  const known = PROVIDER_COLORS[providerId];
  if (known) return known;
  let hash = 0;
  for (const char of providerId) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  return PROVIDER_PALETTE[hash % PROVIDER_PALETTE.length];
}
