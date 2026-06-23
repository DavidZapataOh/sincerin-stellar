/*
 * A tiny, dependency-free client router — exactly two routes (`/` landing,
 * `/demo` app) plus a 404 fallback. The brief allows "React Router or
 * equivalent"; two static routes don't justify a routing dependency (DESIGN.md:
 * "no needless dependencies"), so this is the History-API equivalent: pushState
 * navigation, a `popstate` listener, and a `<Link>` that intercepts plain
 * left-clicks. Hash-free, SSR-free, ~40 lines.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type AnchorHTMLAttributes,
  type ReactNode,
} from "react";

interface RouterCtx {
  path: string;
  navigate: (to: string, opts?: { replace?: boolean }) => void;
}

const Ctx = createContext<RouterCtx | null>(null);

function normalize(path: string): string {
  // Strip a trailing slash (except root) so "/demo/" === "/demo".
  if (path.length > 1 && path.endsWith("/")) return path.slice(0, -1);
  return path;
}

export function RouterProvider({ children }: { children: ReactNode }) {
  const [path, setPath] = useState(() =>
    typeof window === "undefined" ? "/" : normalize(window.location.pathname),
  );

  useEffect(() => {
    const onPop = () => setPath(normalize(window.location.pathname));
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, []);

  const navigate = useCallback(
    (to: string, opts?: { replace?: boolean }) => {
      const next = normalize(to);
      if (next === path) return;
      if (opts?.replace) window.history.replaceState({}, "", next);
      else window.history.pushState({}, "", next);
      setPath(next);
      window.scrollTo({ top: 0, behavior: "auto" });
    },
    [path],
  );

  return <Ctx.Provider value={{ path, navigate }}>{children}</Ctx.Provider>;
}

export function useRouter(): RouterCtx {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useRouter must be used within <RouterProvider>");
  return ctx;
}

/**
 * An in-app link. Intercepts plain left-clicks for client navigation; lets
 * modified clicks (cmd/ctrl/shift/middle), `target="_blank"`, and external URLs
 * fall through to the browser so "open in new tab" still works.
 */
export function Link({
  to,
  children,
  ...rest
}: { to: string } & AnchorHTMLAttributes<HTMLAnchorElement>) {
  const { navigate } = useRouter();
  const external = /^https?:\/\//.test(to) || to.startsWith("mailto:");

  return (
    <a
      href={to}
      onClick={(e) => {
        rest.onClick?.(e);
        if (external || rest.target === "_blank") return;
        if (
          e.defaultPrevented ||
          e.button !== 0 ||
          e.metaKey ||
          e.ctrlKey ||
          e.shiftKey ||
          e.altKey
        )
          return;
        e.preventDefault();
        navigate(to);
      }}
      {...rest}
    >
      {children}
    </a>
  );
}
