<template>
  <div v-if="show" class="back-to-top" @click="scrollToTop" />
</template>

<script setup lang="ts">
// Dashboard pages scroll inside #dashboard-scroll (window never scrolls), so both
// the visibility probe and the jump target that element. Previously window-based (dead).
const scroller = () => document.getElementById('dashboard-scroll');
const scrollTop = ref(0);
const getScrollTop = () => scroller()?.scrollTop || 0;
const scrollToTop = () => scroller()?.scrollTo({ top: 0, behavior: 'smooth' });
const show = computed(() => scrollTop.value > 300);
const onScroll = () => (scrollTop.value = getScrollTop());

onMounted(() => {
  scrollTop.value = getScrollTop();
  scroller()?.addEventListener('scroll', onScroll, { passive: true });
});
onUnmounted(() => scroller()?.removeEventListener('scroll', onScroll));
</script>

<style scoped lang="scss">
.back-to-top {
  position: fixed;
  right: 2.5rem;
  bottom: 2rem;
  z-index: 100;
  width: 2rem;
  height: 1.2rem;
  background-color: var(--primary);
  cursor: pointer;
  mask: url('/svg/back-to-top.svg') no-repeat;
}

.back-to-top:hover {
  background-color: var(--primary-dark);
}

@media (width <= 959px) {
  .back-to-top {
    display: none;
  }
}

@media print {
  .back-to-top {
    display: none;
  }
}
</style>
