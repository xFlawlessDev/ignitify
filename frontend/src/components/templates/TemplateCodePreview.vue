<script setup lang="ts">
import { computed } from "vue";

interface CodeToken {
  text: string;
  className: string;
}

const props = defineProps<{
  content: string;
  language: "toml" | "yaml";
  label: string;
}>();

const highlightedLines = computed(() =>
  props.content.split("\n").map((line) => tokenizeLine(line, props.language)),
);

function tokenizeLine(line: string, language: "toml" | "yaml"): CodeToken[] {
  const pattern =
    language === "yaml"
      ? /(#.*$|"(?:\\.|[^"])*"|'[^']*'|\b(?:true|false|null)\b|\b\d+(?:\.\d+)?\b|[A-Za-z0-9_.-]+(?=\s*:))/g
      : /(#.*$|"(?:\\.|[^"])*"|'[^']*'|\b(?:true|false)\b|\b\d+(?:\.\d+)?\b|^\s*\[[^\]]+\]|[A-Za-z0-9_.-]+(?=\s*=))/g;
  const tokens: CodeToken[] = [];
  let cursor = 0;

  for (const match of line.matchAll(pattern)) {
    const index = match.index ?? cursor;
    if (index > cursor) tokens.push({ text: line.slice(cursor, index), className: "" });

    const text = match[0];
    tokens.push({ text, className: tokenClass(text) });
    cursor = index + text.length;
  }

  if (cursor < line.length) tokens.push({ text: line.slice(cursor), className: "" });
  return tokens.length ? tokens : [{ text: "", className: "" }];
}

function tokenClass(text: string) {
  if (text.startsWith("#")) return "text-muted-foreground/60 italic";
  if (text.startsWith('"') || text.startsWith("'")) return "text-emerald-300";
  if (/^(true|false|null)$/.test(text)) return "text-amber-300";
  if (/^\d/.test(text)) return "text-orange-300";
  return "text-sky-300";
}
</script>

<template>
  <div
    class="max-h-80 overflow-auto rounded-[6px] border border-border bg-[#11161d] focus-visible:outline-2 focus-visible:outline-ring focus-visible:outline-offset-2"
    role="region"
    :aria-label="props.label"
    tabindex="0"
  >
    <pre
      class="min-w-full p-4 font-mono text-xs leading-6 whitespace-pre text-[#d0d5dd]"
    ><code><span
      v-for="(line, lineIndex) in highlightedLines"
      :key="lineIndex"
      class="block min-h-[1.5rem]"
    ><span class="mr-2 inline-block w-8 select-none text-right text-[#667085]">{{ lineIndex + 1 }}</span><span
      v-for="(token, tokenIndex) in line"
      :key="tokenIndex"
      :class="token.className"
    >{{ token.text }}</span></span></code></pre>
  </div>
</template>
