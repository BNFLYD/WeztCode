<script>
  import { afterUpdate } from "svelte";
  import Icon from "@iconify/svelte";

  const STORAGE_KEY = "weztcode_chat_messages";

  let messages = JSON.parse(localStorage.getItem(STORAGE_KEY) || "[]");
  let input_value = "";
  let streaming = false;
  let list_ref;
  let warnings = [];
  let show_warnings = false;
  let models = [];
  let current_model = "";
  let switching = false;

  function save() {
    const raw = JSON.stringify(messages);
    // localStorage tiene ~5MB, pero prevenimos truncamiento
    try {
      localStorage.setItem(STORAGE_KEY, raw);
    } catch {
      // Si excede cuota, guardamos solo los últimos 50 mensajes
      const trimmed = messages.slice(-50);
      localStorage.setItem(STORAGE_KEY, JSON.stringify(trimmed));
    }
  }

  function newConversation() {
    messages = [];
    warnings = [];
    show_warnings = false;
    localStorage.removeItem(STORAGE_KEY);
  }

  afterUpdate(() => {
    if (list_ref) {
      list_ref.scrollTop = list_ref.scrollHeight;
    }
  });

  async function send() {
    const text = input_value.trim();
    if (!text || streaming) return;

    messages = [...messages, { role: "user", content: text }];
    save();
    input_value = "";
    streaming = true;

    const assistant_msg = { role: "assistant", content: "" };
    messages = [...messages, assistant_msg];
    save();

    try {
      const res = await fetch("/api/chat/send", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ message: text }),
      });

      if (!res.ok || !res.headers.get("Content-Type")?.includes("text/event-stream")) {
        const err_text = await res.text();
        assistant_msg.content = `Error del servidor:\n${err_text}`;
        messages = messages;
        save();
        streaming = false;
        return;
      }

      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let buffer = "";

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        buffer = lines.pop() || "";

        for (const line of lines) {
          if (!line.startsWith("data: ")) continue;

          try {
            const data = JSON.parse(line.slice(6));
            switch (data.type) {
              case "token":
                assistant_msg.content += data.content;
                messages = messages;
                save();
                break;
              case "tool_call":
                assistant_msg.content += `\n\n[${data.name}]`;
                messages = messages;
                save();
                break;
              case "warning":
                warnings = [...warnings, data.message];
                break;
              case "error":
                assistant_msg.content += `\n\nError: ${data.message}`;
                messages = messages;
                save();
                break;
              case "done":
                break;
            }
          } catch {}
        }
      }
    } catch (e) {
      assistant_msg.content += `\n\nConnection error: ${e.message}`;
      messages = messages;
      save();
    }

    if (!assistant_msg.content.trim()) {
      assistant_msg.content = "(empty response)";
    }

    streaming = false;
    messages = messages;
    save();
  }

  function handle_keydown(e) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }

  async function load_models() {
    try {
      const res = await fetch("/api/models/list");
      const data = await res.json();
      models = data.data || [];
      if (models.length > 0) {
        const default_model = models.find(m => m.default);
        current_model = default_model ? default_model.name : models[0].name;
      }
    } catch {
      models = [];
    }
  }

  async function switch_model(event) {
    const name = event.target.value;
    if (!name || name === current_model) return;
    switching = true;
    try {
      const res = await fetch("/api/chat/switch-model", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name }),
      });
      const data = await res.json();
      if (data.ok) {
        current_model = name;
      }
    } catch {
      // Si falla, el dropdown se resetea al valor anterior
    }
    switching = false;
  }

  load_models();
</script>

<div class="flex flex-col gap-1 py-2 h-full relative">
  <div
    class="flex items-center gap-2 px-3 py-2 text-sm text-accent-detail/50 border-b border-accent-detail/20 mb-2 flex-shrink-0"
  >
    <Icon icon="ri:search-line" class="w-4 h-4 hover:text-accent-detail" />
    <span class="font-mono truncate flex items-center gap-1 text-print/50">
      Chat
    </span>
    {#if models.length > 0}
      <select
        on:change={switch_model}
        disabled={switching || streaming}
        class="text-[10px] px-1 py-0.5 bg-back-deep border border-accent-detail/30
               rounded text-print/70 outline-none ml-1 max-w-[120px] truncate"
      >
        {#each models as m}
          <option value={m.name} selected={m.name === current_model}>
            {m.name}
          </option>
        {/each}
      </select>
    {/if}
    <div class="ml-auto flex items-center gap-2">
      <button
        class="relative text-xs px-2 rounded border border-accent-detail/30
               hover:bg-accent-detail/10 text-accent-detail/60
               hover:text-accent-detail transition-colors"
        on:click={() => show_warnings = !show_warnings}
      >
        <Icon icon="mdi:alert-outline" class="w-4 h-4" />
        {#if warnings.length > 0}
          <span
            class="absolute -top-1.5 -right-1.5 bg-err text-white text-[10px]
                   rounded-full w-4 h-4 flex items-center justify-center"
          >
            {warnings.length}
          </span>
        {/if}
      </button>
      <button
        class="text-xs px-2 rounded border border-accent-detail/30
               hover:bg-accent-detail/10 text-accent-detail/60
               hover:text-accent-detail transition-colors"
        on:click={newConversation}
      >
        Nueva
      </button>
    </div>
  </div>

  {#if show_warnings && warnings.length > 0}
    <div
      class="mx-3 mb-2 p-2 rounded bg-accent-detail/10 border border-accent-detail/20
             max-h-32 overflow-y-auto text-xs font-mono text-print/70"
    >
      {#each warnings as w, i (i)}
        <div class="py-0.5 border-b border-accent-detail/10 last:border-0">
          ⚠ {w}
        </div>
      {/each}
    </div>
  {/if}

  <div class="flex-1 overflow-y-auto px-3 py-2 space-y-3" bind:this={list_ref}>
    {#if messages.length === 0}
      <div class="flex items-center justify-center h-full">
        <div class="text-center text-print/50">
          <Icon icon="mdi:thinking" class="w-20 h-20 mx-auto mb-3 opacity-50" />
          <p class="text-xl">Animate a crear...</p>
        </div>
      </div>
    {:else}
      {#each messages as msg, i (i)}
        <div
          class={"flex " +
            (msg.role === "user" ? "justify-end" : "justify-start")}
        >
          <div
            class={"max-w-[85%] px-2 py-1 font-semibold rounded-lg text-lg whitespace-pre-wrap " +
              (msg.role === "user"
                ? "bg-accent-detail/95 text-back-deep"
                : "bg-back text-print")}
          >
            {#if msg.role === "assistant" && i === messages.length - 1 && streaming}
              {msg.content}<span class="animate-pulse">▌</span>
            {:else}
              {msg.content}
            {/if}
          </div>
        </div>
      {/each}
    {/if}
  </div>

  <div class="flex flex-col shrink-0">
    <div class="flex mt-auto pt-2 pb-2 w-full items-stretch">
      <div
        class="-mr-2 flex-1 relative px-5 py-6 bg-transparent rounded-l-lg flex items-center justify-center overflow-hidden group"
      >
        <div
          class="rounded-lg absolute inset-0 bg-back -translate-x-[4%] skew-x-[-20deg] origin-left"
        >
          <input
            type="text"
            placeholder="Decime lo que pensás..."
            bind:value={input_value}
            on:keydown={handle_keydown}
            disabled={streaming}
            class="w-full skew-x-[20deg] pl-4 pr-2 bg-transparent rounded-lg text-lg text-print-contrast placeholder:text-print-contrast/50 outline-none"
          />
        </div>
      </div>

      <div
        class="relative pl-5 pr-3 -ml-2 bg-transparent rounded-r-lg flex items-center justify-center overflow-hidden group"
      >
        <button
          class="rounded-lg absolute inset-0 bg-accent-detail translate-x-[14%] skew-x-[-20deg] origin-right"
          aria-label="Enviar"
          on:click={send}
          disabled={streaming}
        ></button>

        <Icon
          icon="mingcute:navigation-fill"
          class="text-back-deep w-6 h-6 relative z-10 transition-transform group-active:scale-75"
        />
      </div>
    </div>
  </div>
</div>
