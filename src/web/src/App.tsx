import { Blocks, FileCheck2, Hash, House, KeyRound, Menu, Network, Pickaxe, X } from "lucide-react";
import { useEffect, useState, type ComponentType } from "react";
import { REPO_URL } from "./config";
import ChainPage from "./pages/ChainPage";
import FileChainPage from "./pages/FileChainPage";
import HashPage from "./pages/HashPage";
import Home from "./pages/Home";
import MiningPage from "./pages/MiningPage";
import NetworkPage from "./pages/NetworkPage";
import TransactionsPage from "./pages/TransactionsPage";

export interface Route {
  path: string;
  chapter: string;
  title: string;
  icon: ComponentType<{ className?: string }>;
  page: ComponentType;
}

/** 網頁的每一頁，跟教材的章節對應。用網址的 # 切換頁面，放在 GitHub Pages 也不用伺服器設定。 */
export const ROUTES: Route[] = [
  { path: "", chapter: "", title: "學習路線", icon: House, page: Home },
  { path: "hash", chapter: "1", title: "雜湊", icon: Hash, page: HashPage },
  { path: "chain", chapter: "2–3", title: "區塊和鏈", icon: Blocks, page: ChainPage },
  { path: "mining", chapter: "4", title: "挖礦和難度", icon: Pickaxe, page: MiningPage },
  { path: "transactions", chapter: "5–6", title: "簽章、交易和出塊", icon: KeyRound, page: TransactionsPage },
  { path: "network", chapter: "7–8", title: "網路和分岔", icon: Network, page: NetworkPage },
  { path: "filechain", chapter: "9", title: "FileChain", icon: FileCheck2, page: FileChainPage },
];

function currentPath() {
  return window.location.hash.replace(/^#\/?/, "");
}

export function useHashRoute(): Route {
  const [path, setPath] = useState(currentPath);
  useEffect(() => {
    const onChange = () => {
      setPath(currentPath());
      window.scrollTo({ top: 0 });
    };
    window.addEventListener("hashchange", onChange);
    return () => window.removeEventListener("hashchange", onChange);
  }, []);
  return ROUTES.find((r) => r.path === path) ?? ROUTES[0];
}

function NavLinks({ active, onNavigate }: { active: Route; onNavigate?: () => void }) {
  return (
    <nav className="space-y-1">
      {ROUTES.map((route) => {
        const Icon = route.icon;
        const isActive = route === active;
        return (
          <a
            key={route.path}
            href={`#/${route.path}`}
            onClick={onNavigate}
            aria-current={isActive ? "page" : undefined}
            className={`flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition ${
              isActive
                ? "bg-indigo-600 text-white"
                : "text-slate-600 hover:bg-slate-200/60 dark:text-slate-300 dark:hover:bg-slate-800"
            }`}
          >
            <Icon className="size-4 shrink-0" />
            <span className="flex-1">{route.title}</span>
            {route.chapter && <span className="text-xs opacity-70">第 {route.chapter} 章</span>}
          </a>
        );
      })}
    </nav>
  );
}

export default function App() {
  const route = useHashRoute();
  const [menuOpen, setMenuOpen] = useState(false);
  const Page = route.page;

  const brand = (
    <a href="#/" className="block">
      <div className="font-mono text-sm font-bold text-indigo-600 dark:text-indigo-400">blockchain-from-scratch</div>
      <div className="text-xs text-slate-500 dark:text-slate-400">從零開始學區塊鏈</div>
    </a>
  );

  return (
    <div className="min-h-screen lg:flex">
      <aside className="hidden w-72 shrink-0 border-r border-slate-200 bg-slate-100/60 p-5 lg:block dark:border-slate-800 dark:bg-slate-900/40">
        <div className="sticky top-5 space-y-6">
          {brand}
          <NavLinks active={route} />
          <p className="text-xs leading-relaxed text-slate-500 dark:text-slate-400">
            網頁上的每個示範都直接執行教材的 Rust 程式（編成 WebAssembly），全部在你的瀏覽器裡跑。
            <a href={REPO_URL} className="mt-1 block text-indigo-600 hover:underline dark:text-indigo-400">
              GitHub 上的原始碼和教材
            </a>
          </p>
        </div>
      </aside>

      <header className="sticky top-0 z-20 flex items-center justify-between border-b border-slate-200 bg-white/90 px-4 py-3 backdrop-blur lg:hidden dark:border-slate-800 dark:bg-slate-950/90">
        {brand}
        <button className="btn-secondary" onClick={() => setMenuOpen((o) => !o)} aria-label="切換選單">
          {menuOpen ? <X className="size-4" /> : <Menu className="size-4" />}
        </button>
      </header>
      {menuOpen && (
        <div className="border-b border-slate-200 bg-white p-3 lg:hidden dark:border-slate-800 dark:bg-slate-950">
          <NavLinks active={route} onNavigate={() => setMenuOpen(false)} />
        </div>
      )}

      <main className="mx-auto w-full max-w-6xl flex-1 px-4 py-8 sm:px-8">
        <Page />
      </main>
    </div>
  );
}
