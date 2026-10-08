import { readFileSync } from "node:fs";
import { expect } from "vitest";

// jsdom runs with `css: false`, so styles.css is never applied and computed
// styles are empty. Layout tests assert on the stylesheet text instead.
// import.meta.dirname (not .url) survives vitest's jsdom transform as the real
// on-disk directory.
let cached: string | undefined;

/** Contents of src/styles.css, read once per test file. */
export function loadStyles(): string {
  if (cached === undefined) {
    if (!import.meta.dirname) {
      throw new Error("import.meta.dirname unavailable to vitest runner");
    }
    cached = readFileSync(`${import.meta.dirname}/../styles.css`, "utf8");
  }
  return cached;
}

/**
 * Declarations of the first rule whose selector is exactly `selector`. The
 * match is anchored to a line start so `.a` cannot match inside `.b .a`.
 */
export function ruleBlock(css: string, selector: string): string {
  const escaped = selector.replace(/[^\w-]/g, "\\$&");
  const match = css.match(
    new RegExp("(?:^|\\r?\\n)" + escaped + "\\s*\\{([^}]*)\\}"),
  );
  expect(match, `no rule for ${selector}`).not.toBeNull();
  return match![1];
}
