<script>
  import { onMount, tick } from "svelte";
  import Icon from "@iconify/svelte";

  const STORAGE_KEY = "weztcode_chat_messages";

  let messages = $state(JSON.parse(localStorage.getItem(STORAGE_KEY) || "[]"));
  let input_value = $state("");
  let streaming = $state(false);
  let list_ref = $state(null);
  let warnings = $state([]);
  let show_warnings = $state(false);
  let models = $state([]);
  let current_model = $state("");
  let switching = $state(false);
  let show_dropdown = $state(false);
  let dropdown_container = $state(null);
  let real_context_percent = $state(null);
  let real_context_window = $state(null);
  let sub_agents = $state([]);
  let builtins = $state([]);
  let current_agent = $state(null);
  let current_icon = $state(null);
  let show_agent_dropdown = $state(false);
  let agent_dropdown_container = $state(null);
  let pending_agent = $state(null);
  let pending_model = $state(null);
  let abort_controller = $state(null);

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

  async function newConversation() {
    try {
      await fetch("/api/chat/new-session", { method: "POST" });
    } catch {}
    messages = [];
    warnings = [];
    show_warnings = false;
    real_context_percent = null;
    real_context_window = null;
    localStorage.removeItem(STORAGE_KEY);
  }

  $effect(() => {
    messages.length;
    const last = messages[messages.length - 1];
    last?.content;
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

    messages = [...messages, { role: "assistant", content: "" }];
    save();

    abort_controller = new AbortController();

    try {
      const res = await fetch("/api/chat/send", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ message: text }),
        signal: abort_controller.signal,
      });

      if (
        !res.ok ||
        !res.headers.get("Content-Type")?.includes("text/event-stream")
      ) {
        const err_text = await res.text();
        messages[messages.length - 1].content = `Error del servidor:\n${err_text}`;
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
                messages[messages.length - 1].content += data.content;
                save();
                break;
              case "tool_call":
                messages[messages.length - 1].content += `\n\n[${data.name}]`;
                save();
                break;
              case "warning":
                warnings = [...warnings, data.message];
                break;
              case "error":
                messages[messages.length - 1].content += `\n\nError: ${data.message}`;
                save();
                break;
              case "session_stats":
                try {
                  const raw = JSON.parse(data.json);
                  const ctx = raw.data?.contextUsage;
                  if (ctx?.percent !== undefined) {
                    real_context_percent =
                      (real_context_percent ?? 0) + ctx.percent;
                  }
                  if (ctx?.contextWindow) {
                    real_context_window = ctx.contextWindow;
                  }
                } catch {}
                break;
              case "done":
                break;
            }
          } catch (e) {
            console.error("[chat] SSE parse error:", e, "line:", line);
          }
        }
      }
    } catch (e) {
      if (e?.name !== 'AbortError') {
        messages[messages.length - 1].content += `\n\nConnection error: ${e.message}`;
        save();
      }
    }

    if (!messages[messages.length - 1].content.trim()) {
      messages[messages.length - 1].content = "(empty response)";
    }

    abort_controller = null;
    streaming = false;
    save();

    if (pending_agent !== null) {
      const agent = pending_agent;
      pending_agent = null;
      await select_agent(agent);
    } else if (pending_model !== null) {
      const model = pending_model;
      pending_model = null;
      await select_model(model);
    }
  }

  function cancel() {
    abort_controller?.abort();
    abort_controller = null;
    streaming = false;
  }

  function handle_keydown(e) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }

  function handle_tab_keydown(e) {
    if (e.key === "Tab" && sub_agents.length > 0) {
      e.preventDefault();
      if (!current_agent) {
        select_agent(e.shiftKey ? sub_agents[sub_agents.length - 1].name : sub_agents[0].name);
        return;
      }
      const idx = sub_agents.findIndex(a => a.name === current_agent);
      if (e.shiftKey) {
        select_agent(sub_agents[idx <= 0 ? sub_agents.length - 1 : idx - 1].name);
      } else {
        select_agent(sub_agents[idx >= sub_agents.length - 1 ? 0 : idx + 1].name);
      }
    }
  }

  async function load_models() {
    try {
      const res = await fetch("/api/models/list");
      const data = await res.json();
      models = data.data || [];
      if (models.length > 0) {
        const default_model = models.find((m) => m.default);
        current_model = default_model ? default_model.name : models[0].name;
      }
    } catch {
      // no-op: models mantiene su valor anterior
    }
  }

  async function load_sub_agents() {
    try {
      const res = await fetch("/api/sub-agents/list");
      const data = await res.json();
      sub_agents = data.data || [];
      if (sub_agents.length > 0) {
        const default_agent = sub_agents.find((a) => a.default);
        if (default_agent) {
          current_agent = default_agent.name;
          current_icon = default_agent.icon || null;
          if (default_agent.model) {
            current_model = default_agent.model;
          }
        }
      }
    } catch {
      sub_agents = [];
    }
    try {
      const res = await fetch("/api/sub-agents/builtins");
      const data = await res.json();
      builtins = data.data || [];
    } catch {
      builtins = [];
    }
  }

  function toggle_dropdown() {
    show_dropdown = !show_dropdown;
  }

  async function select_model(name) {
    if (!name || name === current_model) {
      show_dropdown = false;
      return;
    }
    show_dropdown = false;

    if (streaming) {
      pending_model = name;
      return;
    }

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
        // Model override: clear agent indicator since user chose manually
        current_agent = null;
        current_icon = null;
      }
    } catch {
      // Si falla, el nombre no se actualiza
    }
    switching = false;
  }

  function toggle_agent_dropdown() {
    show_agent_dropdown = !show_agent_dropdown;
    if (show_agent_dropdown) {
      show_dropdown = false;
    }
  }

  async function select_agent(name) {
    show_agent_dropdown = false;
    if (name === current_agent) return;

    if (streaming) {
      pending_agent = name;
      return;
    }

    if (!name) {
      // "ninguno" — switch back to default model without agent
      current_agent = null;
      current_icon = null;
      const default_model = models.find((m) => m.default);
      if (default_model) {
        await select_model(default_model.name);
      }
      return;
    }
    try {
      const res = await fetch("/api/sub-agents/switch", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name }),
      });
      const data = await res.json();
      if (data.ok) {
        current_agent = data.data.agent;
        current_model = data.data.model;
        current_icon = data.data.icon || null;
        await tick();
      }
    } catch {
      // Si falla, no se actualiza
    }
  }

  function handle_click_outside(e) {
    if (
      show_dropdown &&
      dropdown_container &&
      !dropdown_container.contains(e.target)
    ) {
      show_dropdown = false;
    }
    if (
      show_agent_dropdown &&
      agent_dropdown_container &&
      !agent_dropdown_container.contains(e.target)
    ) {
      show_agent_dropdown = false;
    }
  }

  onMount(() => {
    document.addEventListener("click", handle_click_outside);
    document.addEventListener("keydown", handle_tab_keydown);
    return () => {
      document.removeEventListener("click", handle_click_outside);
      document.removeEventListener("keydown", handle_tab_keydown);
    };
  });

  function estimateTokens(text) {
    return Math.ceil(text.length / 4);
  }

  let used_tokens = $derived(
    messages.reduce((sum, msg) => sum + estimateTokens(msg.content), 0),
  );
  let context_limit = $derived(
    models.find((m) => m.name === current_model)?.max_context ?? 4096,
  );
  let context_percent = $derived(
    context_limit > 0 ? (used_tokens / context_limit) * 100 : 0,
  );
  let indicator = $derived(
    Math.round(real_context_percent ?? context_percent),
  );

  async function init() {
    await load_models();
    await load_sub_agents();
  }

  init();
</script>

<div class="flex flex-col gap-1 py-2 h-full relative">
  <div
    class="flex items-center gap-2 px-3 py-2 text-sm text-accent-detail/50 border-b border-accent-detail/20 mb-2 flex-shrink-0"
  >
    <Icon icon="ri:search-line" class="w-4 h-4 hover:text-accent-detail" />
    <span class="font-mono truncate flex items-center gap-1 text-print/50"></span>
    <div class="ml-auto flex items-center gap-2">
      {#if warnings.length > 0}
        <button
          class="relative text-sm px-2 text-print/50 hover:text-print transition-colors group"
          onclick={() => (show_warnings = !show_warnings)}
        >
          <Icon
            icon="mdi:alert-outline"
            class="w-4 h-4 z-10 group-hover:text-print"
          />
          <span
            class="absolute -top-1 -right-1 bg-back font-bold text-print/50 text-[10px] rounded-full w-3 h-3 flex items-center justify-center group-hover:text-print"
          >
            {warnings.length}
          </span>
        </button>
      {/if}

      {#if models.length > 0}
        <div class="relative" bind:this={dropdown_container}>
          <button
            class="px-1 font-mono text-xs {pending_model !== null ? 'text-accent-detail' : 'text-print/50 hover:text-print'} rounded hover:text-print transition-colors max-w-[120px] truncate"
            onclick={toggle_dropdown}
            disabled={switching || streaming}
          >
            {current_model}
            {#if pending_model !== null}
              <span class="ml-1 w-1.5 h-1.5 rounded-full bg-accent-detail animate-pulse" />
            {/if}
          </button>
          {#if show_dropdown}
            <div
              class="absolute top-full right-1 mt-4 z-50 min-w-full bg-back rounded-md shadow-lg py-1"
            >
              {#each models as m}
                <button
                  class="font-semibold block w-full text-left text-sm px-3 py-1.5 whitespace-nowrap rounded-md
                       {m.name === current_model
                    ? 'bg-accent-detail text-back transition-colors'
                    : 'text-print hover:bg-accent-detail/10 transition-colors'}"
                  onclick={() => select_model(m.name)}
                >
                  {m.name}
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/if}

      <button
        class="font-mono text-xs text-print/50 hover:text-print transition-colors"
        onclick={newConversation}
      >
        {indicator}%
      </button>
    </div>
  </div>

  {#if show_warnings && warnings.length > 0}
    <div
      class="mx-3 mb-2 p-2 rounded bg-back max-h-40 overflow-y-auto text-sm font-mono text-print"
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
    <div class="flex mt-auto pt-2 pb-2 w-full items-stretch relative">
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
            onkeydown={handle_keydown}
            disabled={streaming}
            class="w-full skew-x-[20deg] pl-4 pr-2 bg-transparent rounded-lg text-lg text-print-contrast placeholder:text-print-contrast/50 outline-none"
          />
        </div>

        <div class="absolute right-[25px] -bottom-1 z-50">
          <button class="text-print/50 hover:text-print active:scale-10">
            <Icon icon="si:mic-detailed-fill" class="w-4 h-4" />
          </button>
        </div>
      </div>

      {#if sub_agents.length > 0 || current_agent}
        <div
          class="absolute left-0 bottom-2 z-50"
          bind:this={agent_dropdown_container}
        >
          <button
            class="flex items-center gap-1 px-2 py-0.5 text-xs font-semibold font-mono {pending_agent !== null ? 'text-accent-detail' : 'text-print/50 hover:text-print'} transition-colors rounded"
            onclick={toggle_agent_dropdown}
            disabled={streaming}
          >
            <Icon icon={streaming ? "svg-spinners:bars-scale-fade" : current_icon || "simple-icons:pi"} class="w-4 h-4" />
            {current_agent || "default"}
            {#if pending_agent !== null}
              <!-- svelte-ignore element_invalid_self_closing_tag -->
              <span class="ml-1 w-2 h-2 rounded-full bg-accent-detail animate-pulse"></span>
            {/if}
          </button>
          {#if show_agent_dropdown}
            <div
              class="absolute bottom-full left-1 mb-5 z-50 min-w-[140px] bg-back rounded-md shadow-lg py-1"
            >
              {#each sub_agents as agent}
                <button
                  class="font-semibold block w-full text-left text-sm px-3 py-1.5 whitespace-nowrap rounded-md
                       {agent.name === current_agent
                    ? 'bg-accent-detail text-back trasition-colors'
                    : 'text-print hover:bg-accent-detail/10 transition-colors'}"
                  onclick={() => select_agent(agent.name)}
                >
                  {agent.name}
                </button>
              {/each}
              <hr class="border-accent-detail/10 my-1" />
              <button
                class="font-semibold block w-full text-left text-sm px-3 py-1.5 rounded
                       {current_agent === null
                  ? 'bg-accent-detail text-back-deep trasition-colors'
                  : 'text-print hover:bg-accent-detail/10 hover:text-accent/70 transition-colors'}"
                onclick={() => select_agent(null)}
              >
                default
              </button>
            </div>
          {/if}
        </div>
      {/if}

      <div
        class="relative pl-5 pr-3 -ml-2 bg-transparent rounded-r-lg flex items-center justify-center overflow-hidden group"
      >
        <button
          class="rounded-lg absolute inset-0 bg-accent-detail translate-x-[14%] skew-x-[-20deg] origin-right"
          aria-label={streaming ? "Cancelar" : "Enviar"}
          onclick={streaming ? cancel : send}
        ></button>

        <Icon
          icon={streaming ? "mdi:circle" : "mingcute:navigation-fill"}
          class="pointer-events-none text-back-deep w-6 h-6 relative z-10 transition-transform group-active:scale-75"
        />
      </div>
    </div>
  </div>
</div>

<style>
  button:focus-visible {
    outline: none;
  }
</style>