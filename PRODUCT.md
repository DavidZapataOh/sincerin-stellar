# Product

## Register

brand

## Users

**Jueces del hackathon** (Stellar Hacks: Real-World ZK), que entran **de forma asíncrona durante una ventana de ~1 semana** — un desconocido abre un link público cuando quiere, sin la laptop del autor en el medio. Secundariamente, **el operador** (corre el sequencer). El trabajo del juez: conectar su wallet (Freighter, testnet) → **submitear un retiro** → ver el batch agregado liquidar **async** (pending → proving → settled) → **ver sus fondos testnet llegar a su dirección**. La UI es su ventana al pipeline REAL (submit → sequencer → prove → settle on-chain), no a un demo horneado.

## Product Purpose

**Sincerin es el Confidential Payments ROLLUP on Stellar** — la capa de **agregación** que toma N pagos privados (retiradas de un pool de pagos privados base) y los **agrega off-chain en 1 prueba RISC Zero (Groth16/BN254)** verificada **una sola vez on-chain en Soroban** → **N pagos privados liquidan al costo de ~uno**. ⚠️ **Sincerin NO es la privacy pool**: la pool base (`stellar-private-payments` de Nethermind) es el **sustrato** clonado; **Sincerin es el rollup/agregación ENCIMA** — el aporte real (guest RISC Zero + `settle_batch` + benchmark). "Confidential" = **contrapartes unlinkables** (se rompe depósito↔retiro); **montos en claro**. La landing vende **la agregación/escala** (muchos pagos privados → 1 transacción) a quien usaría Sincerin; el demo (UI funcional) prueba que el rollup corre de verdad on-chain.

## Brand Personality

**Técnica · precisa · confiable.** Voz rigurosa, segura por precisión, **honesta** (nunca sobre-afirma). El diseño ES el mensaje: su trabajo es **vender "real on-chain, cero mocks"** con la sobriedad de quien no necesita exagerar. Confianza que se gana mostrando artefactos reales (tx hashes, estado on-chain), no adjetivos.

## Anti-references

- **Terminal / hacker-green-on-black / mono-como-disfraz:** el cliché más quemado de cripto-ZK y **explícitamente prohibido**. Mono solo para DATOS reales (hashes, montos), nunca como estética de toda la UI. (Steer directo del usuario.)
- **Cripto-degen / casino / memecoin:** neón, hype, monedas animadas, "to the moon", gamificación. Lo opuesto al tono.
- **Plantilla SaaS genérica:** body cream/sand, eyebrows en mayúsculas sobre cada sección, hero-metric template, grids de cards idénticas.
- **Fintech corporativo insípido:** azul-marino-y-blanco sin carácter, stock-render, "innovación" vacía.
- Flashy por ser flashy: efectos que no comunican que esto es real y técnico.

## Design Principles

1. **Real, no renderizado** — cada afirmación va respaldada por un artefacto vivo (tx hash, link al explorer, estado on-chain). La UI muestra el pipeline real; jamás finge un resultado. (= el mensaje #1.)
2. **Honesto por construcción** — el copy nunca sobre-afirma: "unlinkable", **nunca** "montos ocultos"; el trust boundary (el operador ve el mapping) se declara, no se esconde. La honestidad es una feature.
3. **La precisión comunica confianza** — números exactos, estados reales, cero hand-waving. El rigor en el detalle es cómo la marca se gana la credibilidad técnica.
4. **La espera ES la prueba** — el proving tarda minutos (~5 medidos); la UX async hace esa latencia **legible y creíble** (es trabajo ZK real), nunca escondida tras un spinner falso.
5. **Cualquiera lo corre** — se optimiza para un desconocido con un link haciendo un retiro real: baja fricción, auto-explicativo, sin pasos manuales.

## Accessibility & Inclusion

WCAG **AA**: contraste de texto ≥4.5:1 (cuerpo) / ≥3:1 (grande). **Estado nunca por color solo** — los estados (pending/proving/settled/failed) llevan icono + etiqueta, no solo un color (defensa para daltonismo). **`prefers-reduced-motion`**: alternativa (crossfade/instantáneo) para toda animación. Navegable por teclado; foco visible.
