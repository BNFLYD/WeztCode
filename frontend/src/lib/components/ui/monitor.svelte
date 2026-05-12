<script>
  import { onMount, onDestroy } from "svelte";
  import { get_colors } from "$lib/theme";

  export let active_section;
  export let on_section_change;

  const sections = [
    { id: "explorer", icon: "solar:code-2-bold", label: "コード" },
    { id: "chat", icon: "mingcute:chat-4-line", label: "チャット" },
    { id: "git", icon: "mynaui:git-commit", label: "GIT" },
    { id: "view", icon: "streamline-plump:web", label: "表示" },
    { id: "term", icon: "devicon-plain:bash", label: "ターミナル" },
    { id: "settings", icon: "hugeicons:settings-03", label: "設定" },
  ];
  // Props in snake_case to distinguish from framework props
  export let active_channel = null;
  export let is_distorting = false;

  let canvas_ref = null;
  let animation_id = null;
  let time = 0;

  onMount(() => {
    if (!canvas_ref) return;

    const ctx = canvas_ref.getContext("2d");
    const width = canvas_ref.width;
    const height = canvas_ref.height;

    const animate = () => {
      // Clear canvas with dark background
      ctx.fillStyle = get_colors('--color-back', '#0d0d0d');
      ctx.fillRect(0, 0, width, height);

      if (is_distorting) {
        // Static/glitch effect
        ctx.fillStyle = get_colors('--color-accent-detail', '#ffffff');
        for (let i = 0; i < 100; i++) {
          ctx.fillRect(
            Math.random() * width,
            Math.random() * height,
            Math.random() * 20 + 5,
            1
          );
        }
        // Random flash effect
        if (Math.random() < 0.1) {
          ctx.fillStyle = `rgba(255, 255, 255, ${Math.random() * 0.3 + 0.1})`;
          ctx.fillRect(0, 0, width, height);
        }
      } else if (!active_channel) {
        // Sine wave animation (default state)
        ctx.strokeStyle = get_colors('--color-accent-detail', '#00ffdd');
        ctx.lineWidth = 2;
        ctx.beginPath();

        const amplitude = height * 0.15;
        const frequency = 0.04;
        const center_y = height / 2;

        for (let x = 0; x < width; x++) {
          const y = center_y + Math.sin((x + time) * frequency) * amplitude;
          if (x === 0) {
            ctx.moveTo(x, y);
          } else {
            ctx.lineTo(x, y);
          }
        }
        ctx.stroke();
      }

      time += 2;
      animation_id = requestAnimationFrame(animate);
    };

    animate();
  });

  onDestroy(() => {
    if (animation_id) {
      cancelAnimationFrame(animation_id);
    }
  });
</script>

<div class="w-full h-full relative">
  <!-- CRT monitor frame -->
  <div
    class="aspect-square rounded-xl border border-accent-detail/50 bg-accent-detail p-1 relative overflow-hidden"
  >
    <!-- Screen container -->
    <div class="w-full h-full rounded-lg relative overflow-hidden bg-back-deep">
      <canvas
        bind:this={canvas_ref}
        width={300}
        height={300}
        class="w-full h-full"
      ></canvas>

      <!-- Slot overlay for channel content -->
      {#if !is_distorting}
        <div
          class="absolute inset-0 font-mono text-sm text-print-contrast flex flex-col"
        >
          <slot />
        </div>
      {/if}

      <!-- CRT scanlines overlay -->
      <div
        class="absolute inset-0 pointer-events-none z-30"
        style="background: repeating-linear-gradient(
          0deg,
          rgba(255, 255, 255, 0.09) 0px,
          rgba(255, 255, 255, 0.09) 0px,
          transparent 1px,
          transparent 6px
        )"
      ></div>

      <!-- CRT curvature vignette -->
      <div
        class="absolute inset-0 pointer-events-none z-30"
        style="background: radial-gradient(ellipse at center, transparent 70%, rgba(0, 0, 0, 0.2) 100%)"
      ></div>
    </div>

    <!-- Monitor label -->
    <div
      class="absolute bottom-2 right-2 text-xs font-specs text-accent-detail"
    >
      {sections.find((s) => s.id === active_section)?.label}
    </div>
  </div>
</div>
