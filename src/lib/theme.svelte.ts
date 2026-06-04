export type Theme = "system" | "light" | "dark";

const KEY = "gitmonitor-theme";
const ORDER: Theme[] = ["system", "light", "dark"];

function load(): Theme {
  const v = localStorage.getItem(KEY);
  return v === "light" || v === "dark" || v === "system" ? v : "system";
}

function apply(t: Theme): void {
  const root = document.documentElement;
  if (t === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", t);
}

/** 테마(system/light/dark) 선택을 localStorage에 영속하고 <html data-theme>로 적용. */
class ThemeStore {
  current = $state<Theme>("system");

  init(): void {
    this.current = load();
    apply(this.current);
  }

  set(t: Theme): void {
    this.current = t;
    localStorage.setItem(KEY, t);
    apply(t);
  }

  /** system → light → dark → system 순환. */
  cycle(): void {
    const i = ORDER.indexOf(this.current);
    this.set(ORDER[(i + 1) % ORDER.length]);
  }

  /** 현재 모드 아이콘. */
  get icon(): string {
    return this.current === "system" ? "🖥️" : this.current === "light" ? "☀️" : "🌙";
  }
}

export const theme = new ThemeStore();
