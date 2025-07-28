import * as monaco from "monaco-editor";

export function registerDiffLanguages() {
  // 注册 diff 语言
  monaco.languages.register({ id: "diff" });

  monaco.languages.setMonarchTokensProvider("diff", {
    defaultToken: "",
    tokenizer: {
      root: [
        [/^@@.*@@/, "meta.diff.range"],
        [/^\+.*/, "markup.inserted.diff"],
        [/^-.*/, "markup.deleted.diff"],
        [/^!.*/, "markup.changed.diff"],
        [/^#.*/, "comment.diff"],
        [/^ .*/, "markup.quote.diff"],
        [/^.*/, "markup.quote.diff"],
      ],
    },
  });

  // 定义 diff 主题
  monaco.editor.defineTheme("diffTheme", {
    base: "vs",
    inherit: true,
    rules: [
      { token: "meta.diff.range", foreground: "6c757d", fontStyle: "italic" },
      { token: "markup.inserted.diff", foreground: "22863a" },
      { token: "markup.deleted.diff", foreground: "cb2431" },
      { token: "markup.changed.diff", foreground: "b08800" },
      { token: "comment.diff", foreground: "6a737d" },
    ],
    colors: {
      "editor.background": "#ffffff",
      "editorGutter.background": "#f8f9fa",
    },
  });
}
