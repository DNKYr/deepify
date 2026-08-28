import tseslint from "typescript-eslint";
import react from "eslint-plugin-react-hooks";
export default tseslint.config({
  ignores: ["dist"],
  extends: [...tseslint.configs.recommended],
  plugins: { "react-hooks": react },
  rules: {
    "react-hooks/rules-of-hooks": "error",
    "react-hooks/exhaustive-deps": "warn",
  },
});
