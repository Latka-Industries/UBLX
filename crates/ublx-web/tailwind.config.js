/** @type {import('tailwindcss').Config} */
const path = require("path");
const { execFileSync } = require("child_process");

/**
 * Scan leptos-shadcn-* crate sources so Toast / Tooltip utilities are kept.
 * Read cargo metadata from stdout (no /tmp file) — large JSON + a full tmpfs
 * previously truncated the write and threw "Unterminated string in JSON".
 */
function leptosShadcnContent() {
  try {
    const raw = execFileSync(
      "cargo",
      [
        "metadata",
        "--format-version=1",
        "--filter-platform=wasm32-unknown-unknown",
        "--manifest-path=Cargo.toml",
      ],
      {
        cwd: path.join(__dirname, "../.."),
        encoding: "utf8",
        maxBuffer: 64 * 1024 * 1024,
        stdio: ["ignore", "pipe", "pipe"],
      },
    );
    const meta = JSON.parse(raw);
    return meta.packages
      .filter((p) => p.name.startsWith("leptos-shadcn-"))
      .map((p) => path.join(path.dirname(p.manifest_path), "src/**/*.rs"));
  } catch (err) {
    console.warn(
      "tailwind: could not resolve leptos-shadcn sources via cargo metadata:",
      err.message,
    );
    return [];
  }
}

module.exports = {
  darkMode: ["class"],
  content: ["./src/**/*.rs", "./index.html", ...leptosShadcnContent()],
  theme: {
    extend: {
      colors: {
        border: "hsl(var(--border) / <alpha-value>)",
        input: "hsl(var(--input) / <alpha-value>)",
        ring: "hsl(var(--ring) / <alpha-value>)",
        background: "hsl(var(--background) / <alpha-value>)",
        foreground: "hsl(var(--foreground) / <alpha-value>)",
        brand: "hsl(var(--brand) / <alpha-value>)",
        primary: {
          DEFAULT: "hsl(var(--primary) / <alpha-value>)",
          foreground: "hsl(var(--primary-foreground) / <alpha-value>)",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary) / <alpha-value>)",
          foreground: "hsl(var(--secondary-foreground) / <alpha-value>)",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive) / <alpha-value>)",
          foreground: "hsl(var(--destructive-foreground) / <alpha-value>)",
        },
        muted: {
          DEFAULT: "hsl(var(--muted) / <alpha-value>)",
          foreground: "hsl(var(--muted-foreground) / <alpha-value>)",
        },
        accent: {
          DEFAULT: "hsl(var(--accent) / <alpha-value>)",
          foreground: "hsl(var(--accent-foreground) / <alpha-value>)",
        },
        card: {
          DEFAULT: "hsl(var(--card) / <alpha-value>)",
          foreground: "hsl(var(--card-foreground) / <alpha-value>)",
        },
        popover: {
          DEFAULT: "hsl(var(--popover) / <alpha-value>)",
          foreground: "hsl(var(--popover-foreground) / <alpha-value>)",
        },
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
    },
  },
  // Our styles/base.css owns the reset; utilities-only keeps ship CSS small.
  corePlugins: {
    preflight: false,
  },
};
