<script lang="ts">
  let { onAddRoot }: { onAddRoot: () => void } = $props();
</script>

<div class="empty">
  <p>스캔할 폴더가 없습니다.</p>
  <button onclick={onAddRoot}>루트 폴더 추가</button>
  <p class="hint">예: ~/Desktop/@Projects</p>
</div>

<style>
  .empty {
    text-align: center;
    padding: 60px 20px;
    color: #666;
  }
  .hint {
    font-size: 0.8em;
    color: #aaa;
  }
</style>
