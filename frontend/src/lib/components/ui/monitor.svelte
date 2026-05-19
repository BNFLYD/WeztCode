<script>
  import { onMount, onDestroy } from "svelte";
  import { get_colors } from "$lib/theme";
  import ExplorerChannel from "./channels/explorer_channel.svelte";

  export let active_section;
  export let on_section_change;
  export let channel = null;
  export let on_channel_close;

  const sections = [
    { id: "explorer", icon: "solar:code-2-bold", label: "コード" },
    { id: "chat", icon: "mingcute:chat-4-line", label: "チャット" },
    { id: "git", icon: "mynaui:git-commit", label: "GIT" },
    { id: "view", icon: "streamline-plump:web", label: "表示" },
    { id: "term", icon: "devicon-plain:bash", label: "ターミナル" },
    { id: "settings", icon: "hugeicons:settings-03", label: "設定" },
  ];
  export let is_distorting = false;

  let canvas_ref = null;
  let animation_id = null;

  let colors = {};

  const update_colors = () => {
    colors = {
      back: get_colors('--color-back', '#0d0d0d'),
      accent: get_colors('--color-accent', '#00ffdd'),
      detail: get_colors('--color-accent-detail', '#ffffff'),
    };
  };

  let is_visible = true;

  const FPS = 30;
  let last_frame = 0;
  let start_time = 0;

  onMount(() => {
    if (!canvas_ref) return;

    update_colors();

    const dpr = devicePixelRatio || 1;
    const rect = canvas_ref.parentElement.getBoundingClientRect();
    canvas_ref.width = rect.width * dpr;
    canvas_ref.height = rect.height * dpr;

    window.addEventListener('theme-change', update_colors);

    const ro = new ResizeObserver(() => {
      const dpr = devicePixelRatio || 1;
      const rect = canvas_ref.parentElement.getBoundingClientRect();
      canvas_ref.width = rect.width * dpr;
      canvas_ref.height = rect.height * dpr;
    });
    ro.observe(canvas_ref.parentElement);

    const observer = new IntersectionObserver(([entry]) => {
      is_visible = entry.isIntersecting;
    });
    observer.observe(canvas_ref);

    const ctx = canvas_ref.getContext("2d");

    const animate = (timestamp) => {
      if (!is_visible) {
        animation_id = requestAnimationFrame(animate);
        return;
      }

      if (timestamp - last_frame < 1000 / FPS) {
        animation_id = requestAnimationFrame(animate);
        return;
      }
      last_frame = timestamp;

      if (!start_time) start_time = timestamp;
      const elapsed = timestamp - start_time;

      const dpr = devicePixelRatio || 1;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

      const css_w = canvas_ref.width / dpr;
      const css_h = canvas_ref.height / dpr;

      ctx.fillStyle = colors.back;
      ctx.fillRect(0, 0, css_w, css_h);

      if (is_distorting) {
        const iw = canvas_ref.width;
        const ih = canvas_ref.height;
        const imageData = ctx.createImageData(iw, ih);
        const data = imageData.data;
        for (let i = 0; i < 100; i++) {
          const x = Math.random() * iw;
          const y = Math.random() * ih;
          const w = Math.random() * 20 * dpr + 5 * dpr;
          for (let px = Math.floor(x); px < Math.floor(x) + w && px < iw; px++) {
            const idx = (Math.floor(y) * iw + px) * 4;
            data[idx] = 255; data[idx+1] = 255; data[idx+2] = 255; data[idx+3] = 255;
          }
        }
        ctx.putImageData(imageData, 0, 0);

        if (Math.random() < 0.1) {
          ctx.fillStyle = `rgba(255, 255, 255, ${Math.random() * 0.3 + 0.1})`;
          ctx.fillRect(0, 0, css_w, css_h);
        }
      } else if (!channel) {
        ctx.strokeStyle = colors.detail;
        ctx.lineWidth = 2;
        ctx.beginPath();

        const amplitude = css_h * 0.15;
        const frequency = 0.04;
        const center_y = css_h / 2;

        for (let x = 0; x < css_w; x++) {
          const y = center_y + Math.sin((x + elapsed * 0.12) * frequency) * amplitude;
          if (x === 0) {
            ctx.moveTo(x, y);
          } else {
            ctx.lineTo(x, y);
          }
        }
        ctx.stroke();
      }

      animation_id = requestAnimationFrame(animate);
    };

    animate();
  });

  onDestroy(() => {
    if (animation_id) {
      cancelAnimationFrame(animation_id);
    }
    window.removeEventListener('theme-change', update_colors);
  });
</script>

<div class="w-full h-full relative rounded-2xl border border-accent-detail/40 bg-accent-detail">
  <div
    class="aspect-square m-1 relative rounded-[14px] bg-accent-detail overflow-hidden"
  >
    <div class="w-full h-full bg-back">
      <canvas
        bind:this={canvas_ref}
        class="w-full h-full"
      ></canvas>

      {#if !is_distorting}
        <div
          class="absolute inset-0 font-mono text-sm text-print-contrast flex flex-col"
        >
          <slot />
        </div>
      {/if}

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

      <div
        class="absolute inset-0 pointer-events-none z-30"
        style="background: radial-gradient(ellipse at center, transparent 70%, rgba(0, 0, 0, 0.2) 100%)"
      ></div>
    </div>

    <div
      class="absolute bottom-2 right-2 text-xs font-specs text-accent-detail"
    >
      {sections.find((s) => s.id === active_section)?.label}
    </div>
  </div>
  {#if channel?.id === 'explorer'}
    <ExplorerChannel
      icon={channel.props.icon}
      name={channel.props.name}
      on_confirm={channel.props.on_confirm}
      on_cancel={channel.props.on_cancel}
      on_close={on_channel_close}
    />
  {/if}
</div>
