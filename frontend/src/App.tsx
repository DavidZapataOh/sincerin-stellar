/*
 * Root — two surfaces, one disciplined system (the s3/02 correction):
 *
 *   /       → Landing   (the brand page; sells the rollup, hands off to /demo)
 *   /demo   → DemoApp   (the functional pipeline you run against the sequencer)
 *
 * The router is a tiny dependency-free History-API wrapper (src/lib/router).
 * The Header is route-aware (a marketing nav on /, a live testnet/wallet bar on
 * /demo). Only /demo talks to the sequencer; the landing is fully static + honest.
 */

import { useRouter } from "./lib/router";
import { Landing } from "./pages/Landing";
import { DemoApp } from "./pages/DemoApp";
import { Header } from "./components/Header";
import { Footer } from "./components/Footer";
import { NotFound } from "./pages/NotFound";

export function App() {
  const { path } = useRouter();

  if (path === "/") {
    return (
      <>
        <Header surface="landing" />
        <Landing />
        <Footer surface="landing" />
      </>
    );
  }

  if (path === "/demo") {
    // DemoApp owns its own header (it carries live network + wallet state) and
    // footer, because both depend on the loaded /config.
    return <DemoApp />;
  }

  return (
    <>
      <Header surface="landing" />
      <NotFound />
      <Footer surface="landing" />
    </>
  );
}
