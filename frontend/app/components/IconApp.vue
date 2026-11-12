<template>
  <!-- Pergame brand mark: abstract P. Same path as public/Logo/Pergame-logo-*.svg, cropped viewBox. -->
  <svg xmlns="http://www.w3.org/2000/svg" width="32" viewBox="140 88 226 404" preserveAspectRatio="xMidYMid meet">
    <path
      :fill="fill || 'var(--text-body)'"
      fill-rule="evenodd"
      d="M150 128 A30 30 0 0 1 210 128 L210 452 A30 30 0 0 1 150 452 Z M178 98 C288 98 356 136 356 205 C356 274 288 312 178 312 Z M254 170 C236 170 222 184 222 202 C222 220 236 234 254 234 L288 234 C306 234 318 220 318 202 C318 184 306 170 288 170 Z"
    />
  </svg>
</template>

<script setup lang="ts">
defineProps<{ fill?: string }>();
</script>
