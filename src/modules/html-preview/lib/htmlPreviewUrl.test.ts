import { describe, expect, it } from "vitest";
import { htmlPreviewUrl, relativeToRoot } from "./htmlPreviewUrl";

const T = "a".repeat(32);

describe("htmlPreviewUrl", () => {
  it("encodes segments and keeps slashes", () => {
    expect(htmlPreviewUrl(T, "my dir/a b.html", "macos")).toBe(
      `terax-preview://${T}/my%20dir/a%20b.html`,
    );
  });
  it("encodes # and ?", () => {
    expect(htmlPreviewUrl(T, "a#b/c?d.html", "linux")).toBe(
      `terax-preview://${T}/a%23b/c%3Fd.html`,
    );
  });
  it("encodes cyrillic and nesting", () => {
    expect(htmlPreviewUrl(T, "папка/x/й.html", "macos")).toBe(
      `terax-preview://${T}/%D0%BF%D0%B0%D0%BF%D0%BA%D0%B0/x/%D0%B9.html`,
    );
  });
  it("uses the loopback host on windows", () => {
    expect(htmlPreviewUrl(T, "a\\b.html", "windows")).toBe(
      `http://terax-preview.localhost/${T}/a/b.html`,
    );
  });
});

describe("relativeToRoot", () => {
  it("strips the root", () => {
    expect(relativeToRoot("/r/a/b.html", "/r")).toBe("a/b.html");
    expect(relativeToRoot("C:\\r\\a.html", "C:/r")).toBe("a.html");
  });
  it("rejects paths outside the root", () => {
    expect(relativeToRoot("/rx/a.html", "/r")).toBeNull();
  });
});
