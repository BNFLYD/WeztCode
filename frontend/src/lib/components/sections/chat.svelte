<script>
  import { afterUpdate } from "svelte";
  import Icon from "@iconify/svelte";

  let messages = [];
  let input_value = "";
  let streaming = false;
  let list_ref;

  afterUpdate(() => {
    if (list_ref) {
      list_ref.scrollTop = list_ref.scrollHeight;
    }
  });

  async function send() {
    const text = input_value.trim();
    if (!text || streaming) return;

    messages = [...messages, { role: "user", content: text }];
    input_value = "";
    streaming = true;

    const assistant_msg = { role: "assistant", content: "" };
    messages = [...messages, assistant_msg];

    try {
      const res = await fetch("/api/chat/send", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ message: text }),
      });

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
                break;
              case "tool_call":
                assistant_msg.content += `\n\n[${data.name}]`;
                messages = messages;
                break;
              case "error":
                assistant_msg.content += `\n\nError: ${data.message}`;
                messages = messages;
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
    }

    if (!assistant_msg.content.trim()) {
      assistant_msg.content = "(empty response)";
    }

    streaming = false;
    messages = messages;
  }

  function handle_keydown(e) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }
</script>

<div class="flex flex-col h-full">
  <div
    class="flex-1 overflow-y-auto px-3 py-2 space-y-3"
    bind:this={list_ref}
  >
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
          class={"flex " + (msg.role === "user" ? "justify-end" : "justify-start")}
        >
          <div
            class={
              "max-w-[85%] px-2 py-2 rounded-lg text-lg whitespace-pre-wrap " +
              (msg.role === "user"
                ? "bg-accent-detail/80 text-back"
                : "bg-back text-print")
            }
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
