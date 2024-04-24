import { defineConfig } from 'tsup';
import dotenv from 'dotenv';

const env = dotenv.config().parsed;

// Convert environment variables to an object for replacement
const envKeys = Object.keys(env ?? []).reduce(
  (prev: Record<string, string>, next) => {
    const name = `process.env.${next}`;
    prev[name] = JSON.stringify(env![next]);
    return prev;
  },
  {}
);

export default defineConfig((options) => ({
  minify: !options.watch,
  splitting: false,
  sourcemap: true,
  dts: true,
  clean: true,
  format: ['esm', 'cjs'],
  esbuildOptions(esbOptions) {
    esbOptions.define = envKeys;
  }
}));
