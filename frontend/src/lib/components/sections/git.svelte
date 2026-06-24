<script context="module">
  let saved_state = { tool_index: 0, cursor: 0 };
  let saved_tool_cursors = {};
</script>

<script>
  import { afterUpdate, onDestroy, onMount } from "svelte";
  import Icon from "@iconify/svelte";

  export let active_section = "git";

  let tools = [];
  let active_tool_index = 0;
  let items = [];
  let summary = null;
  let running = false;
  let loading = true;
  let error = null;
  let cursor_index = 0;
  let list_ref;
  let controller = null;

  onMount(() => {
    load_tools();
  });

  async function load_tools() {
    if (controller) controller.abort();
    controller = new AbortController();
    const signal = controller.signal;
    loading = true;
    error = null;

    try {
      const res = await fetch("/api/analyze/tools", { signal });
      const json = await res.json();
      if (json.ok) {
        tools = json.data.tools || [];

        if (saved_state.tool_index !== null && saved_state.tool_index < tools.length) {
          active_tool_index = saved_state.tool_index;
        }

        if (saved_tool_cursors[active_tool_index] !== undefined) {
          cursor_index = saved_tool_cursors[active_tool_index];
        }

        if (tools.length > 0) {
          run_current_tool();
        }
      } else {
        error = json.error;
      }
    } catch (e) {
      if (e.name !== "AbortError") error = e.message;
    }
    if (!signal.aborted) loading = false;
  }

  async function run_current_tool() {
    if (tools.length === 0) return;
    await run_tool(tools[active_tool_index].id);
  }

  async function run_tool(tool_id) {
    if (controller) controller.abort();
    controller = new AbortController();
    const signal = controller.signal;
    running = true;
    error = null;
    items = [];
    summary = null;
    cursor_index = 0;

    try {
      const res = await fetch("/api/analyze/run", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ tool_id }),
        signal,
      });
      const json = await res.json();
      if (json.ok) {
        items = json.data.items || [];
        summary = json.data.summary || null;
      } else {
        error = json.error;
      }
    } catch (e) {
      if (e.name !== "AbortError") error = e.message;
    }
    if (!signal.aborted) {
      running = false;
    }
  }

  function switch_tool(index) {
    if (index < 0 || index >= tools.length || index === active_tool_index) return;
    saved_tool_cursors[active_tool_index] = cursor_index;
    active_tool_index = index;

    if (saved_tool_cursors[active_tool_index] !== undefined) {
      cursor_index = saved_tool_cursors[active_tool_index];
    } else {
      cursor_index = 0;
    }

    if (items.length === 0 && !running && !loading) {
      run_current_tool();
    }
    scroll_to_cursor();
  }

  function cancel_run() {
    if (controller) {
      controller.abort();
      controller = null;
    }
    running = false;
  }

  function move_cursor(delta) {
    const new_index = cursor_index + delta;
    if (new_index < 0 || new_index >= items.length) return;
    cursor_index = new_index;
    scroll_to_cursor();
  }

  function scroll_to_cursor() {
    if (!list_ref) return;
    const child = list_ref.querySelector(`[data-index="${cursor_index}"]`);
    if (child) child.scrollIntoView({ block: "nearest" });
  }

  function open_item(index) {
    const item = items[index];
    if (!item) return;

    if (item.type === "test" && item.status === "fail" && item.file) {
      const line = item.line || 1;
      fetch(`/api/editor/open?path=${encodeURIComponent(item.file)}`);
    } else if (item.type === "diagnostic" && item.file) {
      const line = item.line || 1;
      fetch(`/api/editor/open?path=${encodeURIComponent(item.file)}`);
    }
  }

  function item_class(index) {
    if (cursor_index === index) {
      return "bg-accent/10";
    }
    return "hover:bg-accent/5";
  }

  function item_status_icon(item) {
    if (item.type === "test") {
      if (item.status === "pass") return "check";
      if (item.status === "fail") return "x";
      return "minus";
    }
    if (item.type === "diagnostic") {
      if (item.severity === "error") return "x";
      if (item.severity === "warning") return "alert-triangle";
      return "info";
    }
    return "circle";
  }

  function item_status_color(item) {
    if (item.type === "test") {
      if (item.status === "pass") return "text-accent";
      if (item.status === "fail") return "text-accent-err";
      return "text-print/50";
    }
    if (item.type === "diagnostic") {
      if (item.severity === "error") return "text-accent-err";
      if (item.severity === "warning") return "text-accent-warn";
      return "text-accent-detail";
    }
    return "text-print";
  }

  function item_label(item) {
    if (item.type === "test") {
      let label = item.name || "";
      if (item.suite) {
        label = `${item.suite}::${item.name}`;
      }
      return label;
    }
    if (item.type === "diagnostic") {
      if (item.file) {
        let loc = item.file;
        if (item.line) loc += `:${item.line}`;
        if (item.col) loc += `:${item.col}`;
        return loc;
      }
      return (item.code || "");
    }
    return "";
  }

  function has_tests() {
    return items.some(i => i.type === "test");
  }

  function has_diagnostics() {
    return items.some(i => i.type === "diagnostic");
  }

  function test_items() {
    return items.filter(i => i.type === "test");
  }

  function diagnostic_items() {
    return items.filter(i => i.type === "diagnostic");
  }

  function original_index(filtered_index, filtered_array) {
    const item = filtered_array[filtered_index];
    return items.indexOf(item);
  }

  function handle_keydown(e) {
    if (!document.hasFocus()) return;
    if (active_section !== "git") return;

    if (running) {
      if (e.key === "Escape") {
        e.preventDefault();
        cancel_run();
      }
      return;
    }

    switch (e.key) {
      case "j":
      case "ArrowDown":
        e.preventDefault();
        move_cursor(1);
        break;
      case "k":
      case "ArrowUp":
        e.preventDefault();
        move_cursor(-1);
        break;
      case "l":
      case "Enter":
        e.preventDefault();
        open_item(cursor_index);
        break;
      case "r":
      case "R":
        e.preventDefault();
        run_current_tool();
        break;
      case "Tab":
        e.preventDefault();
        if (e.shiftKey) {
          switch_tool((active_tool_index - 1 + tools.length) % tools.length);
        } else {
          switch_tool((active_tool_index + 1) % tools.length);
        }
        break;
      case "Escape":
        e.preventDefault();
        if (error) error = null;
        break;
      case "1":
        if (tools.length >= 1) { e.preventDefault(); switch_tool(0); }
        break;
      case "2":
        if (tools.length >= 2) { e.preventDefault(); switch_tool(1); }
        break;
      case "3":
        if (tools.length >= 3) { e.preventDefault(); switch_tool(2); }
        break;
    }
  }

  afterUpdate(() => {
    if (!loading && !running && list_ref && items.length > 0) {
      scroll_to_cursor();
    }
  });

  onDestroy(() => {
    saved_state = { tool_index: active_tool_index, cursor: cursor_index };
    saved_tool_cursors[active_tool_index] = cursor_index;
    if (controller) controller.abort();
  });
</script>

<svelte:window on:keydown={handle_keydown} />

<div class="flex flex-col gap-1 py-2 h-full relative">
  <div class="flex items-center gap-2 px-3 py-2 text-sm text-accent-detail/50 border-b border-accent-detail/20 mb-2 flex-shrink-0">
    {#each tools as tool, index}
      <button
        on:click={() => switch_tool(index)}
        class="flex items-center gap-1.5 px-2 py-1 rounded-md transition-colors {active_tool_index === index
          ? 'bg-accent/15 text-accent'
          : 'text-print/50 hover:text-print hover:bg-accent/5'}"
      >
        <Icon icon={tool.icon} class="w-4 h-4" />
        <span class="text-xs font-semibold">{tool.name}</span>
      </button>
    {/each}

    <div class="flex-1"></div>

    {#if tools.length > 0}
      <button
        on:click={run_current_tool}
        disabled={running}
        class="flex items-center gap-1 text-xs text-print/50 hover:text-print transition-colors disabled:opacity-30"
      >
        <Icon icon={running ? "tabler:loader-2" : "tabler:player-play"} class="w-4 h-4 {running ? 'animate-spin' : ''}" />
      </button>
    {/if}
  </div>

  <div class="flex-1 overflow-y-auto min-h-0" bind:this={list_ref}>
    {#if loading}
      <div class="flex items-center justify-center py-8">
        <span class="text-print/50 text-lg">Loading...</span>
      </div>
    {:else if error}
      <div class="flex items-center justify-center py-8">
        <span class="text-accent-err text-lg">{error}</span>
      </div>
    {:else if running}
      <div class="flex items-center justify-center py-8">
        <span class="text-print/50 text-lg">Running...</span>
      </div>
    {:else if items.length === 0}
      <div class="flex items-center justify-center py-8">
        <span class="text-print/50 text-lg">Sin resultados</span>
      </div>
    {:else}
      {#if has_tests()}
        <div class="px-3 py-1 text-xs font-bold text-print/30 uppercase tracking-wide">
          Tests
        </div>
        {#each test_items() as item, fi (item.name + item.suite)}
          <!-- svelte-ignore a11y-click-events-have-key-events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="flex flex-col px-3 py-1.5 rounded-lg transition-colors cursor-pointer {item_class(original_index(fi, test_items()))}"
            data-index={original_index(fi, test_items())}
            on:click={() => {
              cursor_index = original_index(fi, test_items());
              open_item(cursor_index);
            }}
          >
            <div class="flex items-center gap-2">
              <span class="{item_status_color(item)} shrink-0">
                {#if item.status === "pass"}
                  <Icon icon="tabler:circle-check-filled" class="w-4 h-4" />
                {:else if item.status === "fail"}
                  <Icon icon="tabler:circle-x-filled" class="w-4 h-4" />
                {:else}
                  <Icon icon="tabler:circle-minus" class="w-4 h-4" />
                {/if}
              </span>
              <span class="text-print text-sm truncate">{item_label(item)}</span>
            </div>
            {#if item.status === "fail" && item.message}
              <div class="ml-7 mt-0.5 text-xs text-accent-err/80 whitespace-pre-wrap line-clamp-3">
                {item.message}
              </div>
            {/if}
          </div>
        {/each}
      {/if}

      {#if has_tests() && has_diagnostics()}
        <div class="border-t border-accent-detail/10 my-1"></div>
      {/if}

      {#if has_diagnostics()}
        <div class="px-3 py-1 text-xs font-bold text-print/30 uppercase tracking-wide">
          Diagnostics
        </div>
        {#each diagnostic_items() as item, di (item.file + item.line + item.col)}
          <!-- svelte-ignore a11y-click-events-have-key-events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="flex flex-col px-3 py-1.5 rounded-lg transition-colors cursor-pointer {item_class(original_index(di, diagnostic_items()))}"
            data-index={original_index(di, diagnostic_items())}
            on:click={() => {
              cursor_index = original_index(di, diagnostic_items());
              open_item(cursor_index);
            }}
          >
            <div class="flex items-center gap-2">
              <span class="{item_status_color(item)} shrink-0">
                {#if item.severity === "error"}
                  <Icon icon="tabler:x-circle" class="w-4 h-4" />
                {:else if item.severity === "warning"}
                  <Icon icon="tabler:alert-triangle" class="w-4 h-4" />
                {:else}
                  <Icon icon="tabler:info-circle" class="w-4 h-4" />
                {/if}
              </span>
              <span class="text-print text-sm truncate">{item_label(item)}</span>
              {#if item.code}
                <span class="text-xs text-accent-detail/50 shrink-0">{item.code}</span>
              {/if}
            </div>
            {#if item.message}
              <div class="ml-7 mt-0.5 text-xs text-print/60 whitespace-pre-wrap line-clamp-3">
                {item.message}
              </div>
            {/if}
          </div>
        {/each}
      {/if}
    {/if}
  </div>

  {#if summary}
    <div class="flex items-center gap-3 px-3 py-2 border-t border-accent-detail/20 text-xs text-print/50 flex-shrink-0">
      {#if summary.passed > 0 || summary.failed > 0}
        <span class="text-accent">
          <Icon icon="tabler:circle-check-filled" class="w-3 h-3 inline mr-0.5" />
          {summary.passed}
        </span>
        <span class="text-accent-err">
          <Icon icon="tabler:circle-x-filled" class="w-3 h-3 inline mr-0.5" />
          {summary.failed}
        </span>
      {/if}
      {#if summary.errors > 0}
        <span class="text-accent-err">
          <Icon icon="tabler:x-circle" class="w-3 h-3 inline mr-0.5" />
          {summary.errors}
        </span>
      {/if}
      {#if summary.warnings > 0}
        <span class="text-accent-warn">
          <Icon icon="tabler:alert-triangle" class="w-3 h-3 inline mr-0.5" />
          {summary.warnings}
        </span>
      {/if}
    </div>
  {/if}
</div>

<style>
  button:focus-visible {
    outline: none;
  }
</style>
