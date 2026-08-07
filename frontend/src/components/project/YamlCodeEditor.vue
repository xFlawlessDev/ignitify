<script setup lang="ts">
import { computed, shallowRef } from "vue";

interface CodeToken {
  text: string;
  className: string;
}

const props = defineProps<{
  id: string;
  placeholder?: string;
  ariaLabel?: string;
  required?: boolean;
}>();

const model = defineModel<string>({ required: true });
const scroll = shallowRef({ top: 0, left: 0 });

const highlightedLines = computed(() =>
  model.value.split("\n").map((line) => tokenizeYamlLine(line)),
);

const highlightStyle = computed(() => ({
  transform: `translate(${-scroll.value.left}px, ${-scroll.value.top}px)`,
}));

function tokenizeYamlLine(line: string): CodeToken[] {
  const tokens: CodeToken[] = [];
  const pattern =
    /(#.*$|"(?:\\.|[^"])*"|'[^']*'|\b(?:true|false|null)\b|\b\d+(?:\.\d+)?\b|[A-Za-z0-9_.-]+(?=\s*:))/g;
  let cursor = 0;

  for (const match of line.matchAll(pattern)) {
    const index = match.index ?? cursor;
    if (index > cursor) tokens.push({ text: line.slice(cursor, index), className: "" });

    const text = match[0];
    const className = text.startsWith("#")
      ? "text-muted-foreground/60 italic"
      : text.startsWith('"') || text.startsWith("'")
        ? "text-emerald-300"
        : /^(true|false|null)$/.test(text)
          ? "text-amber-300"
          : /^\d/.test(text)
            ? "text-orange-300"
            : "text-sky-300";
    tokens.push({ text, className });
    cursor = index + text.length;
  }

  if (cursor < line.length) tokens.push({ text: line.slice(cursor), className: "" });
  return tokens.length ? tokens : [{ text: "", className: "" }];
}

function syncScroll(event: Event) {
  const target = event.target as HTMLTextAreaElement;
  scroll.value = { top: target.scrollTop, left: target.scrollLeft };
}
</script>

<template>
  <div class="relative min-h-[420px] overflow-hidden border border-border bg-[#11161d]">
    <pre
      class="pointer-events-none absolute top-0 left-0 min-w-full p-4 font-mono text-xs leading-6 whitespace-pre"
      :style="highlightStyle"
      aria-hidden="true"
    ><span
      v-for="(line, lineIndex) in highlightedLines"
      :key="lineIndex"
      class="block min-h-[1.5rem]"
    ><span class="mr-2 inline-block w-8 select-none text-right text-[#667085]">{{ lineIndex + 1 }}</span><span
      v-for="(token, tokenIndex) in line"
      :key="tokenIndex"
      :class="token.className"
    >{{ token.text }}</span></span></pre>
    <textarea
      :id="props.id"
      v-model="model"
      :aria-label="props.ariaLabel"
      :placeholder="props.placeholder"
      :required="props.required"
      class="absolute inset-0 min-h-[420px] w-full resize-y overflow-auto bg-transparent p-4 pl-14 font-mono text-xs leading-6 whitespace-pre text-transparent caret-sky-300 outline-none placeholder:text-[#667085] selection:bg-sky-300/20"
      style="color: transparent; -webkit-text-fill-color: transparent"
      wrap="off"
      spellcheck="false"
      @scroll="syncScroll"
    />
  </div>
</template>
