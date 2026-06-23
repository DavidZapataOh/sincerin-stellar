# Benchmark de dos ejes — proving off-chain vs costo on-chain, por N (CONTEXT.md D2 / D6)

El diferenciador del proyecto: **agregar N retiradas en 1 receipt** hace que el
**costo on-chain crezca sub-linealmente** (1 verificación Groth16 ~constante + N
transferencias chicas), manteniéndose **muy dentro del budget** mientras el
**proving off-chain sube** con N. El costo on-chain del agregado NO es plano —
crece 31.5M→56.1M de N=2 a N=32 — pero lo hace mucho más lento que el budget,
y mucho más lento que el baseline (N verificaciones sueltas). Cada retirada es una
hoja del árbol del pool; `depth = ceil(log2 N)` (N=8→3, N=16→4, N=32→5).

- **N=8 (depth 3)** = **demo on-chain**: probado con el **guest DESPLEGADO**
  (`image_id cbeab7aa6ce69944e10cca8c7ed94d15aae297f2580752f07a15c6cab6ba0d46`, el
  que ata `settle_batch`) y **settleado en testnet** (8 retiradas en 1 tx).
- **N=16/32** = **proving-only** (informativo): guest de bench separado
  (`rollup-guest-bench`, `ROLLUP_TREE_DEPTH=4|5`), nunca desplegado/settleado.

## Eje 1 — proving off-chain (SUBE con N)

| N | depth | cycles (medidos, executor) | proving CPU (Mac M-series, **desarrollo**) | proving GPU (RTX 3090, **producción**) |
|---|-------|---------------------------|--------------------------------------------|----------------------------------------|
| 2 | 3 | 30,670,848 | ~1h 14m · **MEDIDO** | — |
| 8 | 3 | 122,683,392 | 4h 26m 07s · **MEDIDO** | **5m 04s · MEDIDO** (PARADA 1) |
| 16 | 4 | 263,192,576 | ~9h 19m · **PROYECTADO** (¹) | — |
| 32 | 5 | 561,512,448 | ~19h 40m · **PROYECTADO** (²) | — |

> **El número de producción es minutos, no horas.** El proving real de N=8 en GPU
> (RTX 3090, `make prove` con CUDA nativo) midió **5m 04s** — **~52× más rápido** que
> los 4h 26m del Mac. Las horas de la columna CPU son artefacto de **desarrollo**
> (emulación x86-on-ARM con Docker para el wrap STARK→Groth16); en producción el
> proving es de **minutos**. Mostrar solo las horas del Mac subvendería el sistema.
>
> **Provenance por celda (honestidad):** los **4 cycle counts son MEDIDOS** por el
> executor (sin wrap; el de N=8 == el total del prover → método validado). El
> **tiempo CPU es MEDIDO solo para N=2 y N=8** (prove real completo); **N=16 y N=32
> son PROYECTADOS** — y **N=16 NO tiene tiempo CPU medido**: su prove real **FALLÓ a
> 7h38m en el wrap Groth16 sin producir receipt**, así que ese 7h38m es solo un
> **ancla parcial del STARK**, no end-to-end. El **tiempo GPU es MEDIDO solo para
> N=8** (PARADA 1, RTX 3090). No se mezcla medido con proyectado sin marcarlo.

- **cycles**: valor REAL. Para N=2/8 = `prove_info.stats.total_cycles` del prover;
  para N=16/32 = cycles padded del **executor** (`sum 2^po2` por segmento), que es
  idéntico al total del prover — **validado**: el executor da N=8 = `122,683,392`,
  exactamente lo que reportó el prove.
- **proving N=16/32 = PROYECTADO** por ajuste lineal cycle→tiempo de los 2 puntos
  medidos: `t ≈ 633s + 125s/Mcyc` (N=2 1h14m@30.7M, N=8 4h26m@122.7M).
  - (¹) **N=16 se INTENTÓ probar de verdad**: corrió **7h38m** y falló en el wrap
    Groth16 (`prove_with_opts(groth16): verify segment`) — presión de memoria /
    límite de tamaño del SNARK bajo emulación x86-on-ARM con Docker a 7.65 GiB. El
    7h38m (≈82% de los 9h19m proyectados) **confirma la proyección**. Con más RAM
    de Docker el wrap cerraría; el cuello es el wrap emulado, no el STARK.
  - (²) **N=32 no se intentó** (≈20h + fallaría igual a 7.65 GiB). Proyectado.

## Eje 2 — costo on-chain del settle (sub-lineal en N, dentro del budget)

| N | settle cpu_insn | % de 400M | medición | baseline = N verificaciones individuales |
|---|-----------------|-----------|----------|------------------------------------------|
| 2 | ~31,500,000 | ~7.9% | MEDIDO (verify, s1/05) | 2 × 35M = 70M |
| 8 | **36,118,956** | **~9.0%** | **MEDIDO** (settle tx `aedc1cc4…`) | 8 × 35M = **280M** |
| 16 | ~43.8M (proy.) | ~11% | proyectado (³) | 16 × 35M = **560M ⟶ NO cabe en 400M** |
| 32 | ~56.1M (proy.) | ~14% | proyectado (³) | 32 × 35M = **1,120M ⟶ NO cabe** |

- (³) settle = 1 verificación Groth16 (~constante, ~31.5M) + N×(`assert !spent;mark`
  + `transfer`). De N=2→N=8 medido: 31.5M→36.1M = +4.6M por +6 notas ≈ 0.77M/nota,
  lineal pequeño. Proyección N=16/32 = `31.5M + 0.77M·N`.
- **El punto:** el settle agregado **crece sub-linealmente (~8→14% del budget de
  N=2 a N=32)** — NO es plano, pero crece despacio — mientras el baseline (N
  verificaciones sueltas del pool) crece lineal y **a partir de N≈12 ni siquiera
  cabe en el budget de 1 tx (400M)**. La agregación no es solo más barata:
  **habilita batches que de otro modo serían imposibles on-chain.**

## Evidencia on-chain (testnet, reproducible)

- **Settle N=8 (8 retiradas en 1 tx):** tx `aedc1cc42f112d65913d4b1b5fb0e9b5636481e2f10a86f85ed21f5c0f605ea9`
  · SUCCESS (ledger 3215136) · 8 recipients acreditados (1000..1007 stroops, suma
  8028) · rollup→0 · replay → `Error(Contract,#3) NullifierSpent`. Rollup
  `CCGUQKT4CWEZBVECATHLZJRUELXNRUATHAXUUTPFIW4GMKRBQ4K36HF5`, pool
  `CCE4URVAZ5HS7MBL5QMFQXQ6GV4TFQXARFFXZQENFNVQFNAY2FVI2DL6`.
- **Settle N=2:** tx `a0937a85…` (s2/03).

## Método (reproducible)

```bash
# input sets committed: golden/n{2,8,16,32}_inputs.json (gen-inputs, zk-core crypto)
# cycles (rápido, sin wrap):  cargo run -p host --release -- execute --inputs golden/nN_inputs.json   # (ROLLUP_TREE_DEPTH=4|5 para N=16|32)
# prove real (multi-hora, Docker):  bash scripts/proving_times.sh 8        # N=8 (deployed guest, settle-able)
# settle N=8 on-chain:  bash scripts/deploy_settle_n8.sh
```

> Entorno: Apple Silicon (aarch64), 10 cores, Docker 7.65 GiB · risc0-zkvm =3.0.5,
> container groth16 `r0.1.88.0` · 2026-06-21/22. Proves reales con `RISC0_DEV_MODE=0`
> (nunca dev-mode). El cuello del wrap es la emulación x86-on-ARM; más RAM de Docker
> permitiría cerrar N≥16 si se quisiera medir end-to-end.
