import globals from "globals";
import pluginJs from "@eslint/js";
import tseslintParser from "@typescript-eslint/parser";
import tseslintPlugin from "@typescript-eslint/eslint-plugin";
import pluginReact from "eslint-plugin-react";

export default [
  { 
    files: ["**/*.{js,mjs,cjs,ts,jsx,tsx}"],
    ignores: ["dist/**", "src-tauri/target/**"],
  },
  {
    languageOptions: {
      parser: tseslintParser,
      parserOptions: {
        ecmaFeatures: {
          jsx: true,
        },
        ecmaVersion: "latest",
        sourceType: "module",
      },
      globals: { ...globals.browser, ...globals.node },
    },
  },
  pluginJs.configs.recommended,
  pluginJs.configs.recommended,
  {
    plugins: {
      "@typescript-eslint": tseslintPlugin,
    },
    rules: tseslintPlugin.configs.recommended.rules,
  },
  {
    plugins: {
      react: pluginReact,
    },
    settings: {
      react: {
        version: "detect"
      }
    },
    rules: {
      ...pluginReact.configs.recommended.rules,
      ...pluginReact.configs["jsx-runtime"].rules,
    },
  },
  {
    rules: {
      // Add any custom rules here
    },
  },
];