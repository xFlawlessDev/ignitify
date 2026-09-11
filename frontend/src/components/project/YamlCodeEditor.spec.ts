// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { createApp, nextTick, ref } from "vue";

afterEach(() => {
  document.body.replaceChildren();
});

describe("YamlCodeEditor", () => {
  it("keeps the editable text layer aligned with the highlighted preview", async () => {
    const component = (await import("./YamlCodeEditor.vue")).default;
    const value = ref("services:\n  web:\n    image: nginx:1.27");
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp({
      components: { YamlCodeEditor: component },
      setup() {
        return { value };
      },
      template: '<YamlCodeEditor id="compose" v-model="value" />',
    });
    app.mount(host);
    await nextTick();

    const preview = host.querySelector("pre") as HTMLPreElement;
    const editor = host.querySelector("textarea") as HTMLTextAreaElement;
    expect(preview.className).toContain("font-mono");
    expect(preview.className).toContain("text-xs");
    expect(preview.className).toContain("leading-6");
    expect(editor.className).toContain("font-mono");
    expect(editor.className).toContain("text-xs");
    expect(editor.className).toContain("md:text-xs");
    expect(editor.className).not.toContain("md:text-sm");
    expect(editor.className).toContain("leading-6");
    expect(editor.className).toContain("border-0");

    Object.defineProperties(editor, {
      scrollLeft: { configurable: true, value: 36 },
      scrollTop: { configurable: true, value: 48 },
    });
    editor.dispatchEvent(new Event("scroll"));
    await nextTick();

    expect(preview.style.transform).toBe("translate(-36px, -48px)");
    app.unmount();
  });
});
