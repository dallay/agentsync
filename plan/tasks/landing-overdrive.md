# Landing overdrive A — revelado por scroll (toda la landing, movimiento)

Ruta: Direct inline (in-thread; subagents del skill no disponibles en este harness).
Comando: overdrive, dirección A elegida por el usuario (revelado por scroll, CSS puro).
Alcance: toda la landing. Acento: más movimiento. Mundo: El Terminal Luminoso se preserva.

## Sistema

- Un solo momento autoral: coreografía de revelado ligada al scroll.
- Hero: deriva sutil de glows/grid (`scroll()`, 0→80vh, solo transform+opacidad).
- Features: heading + 7 cards con `view()`, rise 28px + opacidad, ease-out exponencial,
  stagger pequeño por `animation-delay`. Sin blur (perf).
- Quick Start: solo el heading `#quick-start` con el mismo rise (selector seguro,
  único en `index.mdx`). Los Tabs se dejan intactos (interacción propia).
- Enhancement progresivo: estado base visible; movimiento solo en
  `@supports (animation-timeline: ...)`. Firefox/sin soporte ve estático bueno.
- Tokens `--as-*` solo; sin sombras en reposo; sin nuevos roles de color;
  targets 44px y `:focus-visible` intactos.
- Reduced motion: nuevos selectores al bloque existente (animation none + restore
  opacity 1 / transform none).

## Tareas

- [x] Sistema establecido + dirección A confirmada (pregunta estructurada)
- [ ] Escribir bloque scroll-reveal en `custom.css` (antes del bloque reduced-motion)
- [ ] Extender bloque reduced-motion con los nuevos selectores
- [ ] `docs:build` + detector (una pasada) + inspección desktop+móvil junta
- [ ] Un batch de fixes, máx una ronda de confirmación, cerrar

## Evidencia

- RESOLVED_CONTEXT: target `src/content/docs/index.mdx` existe, plataforma `web`
- `Features`/`FeatureItem` solo se usan en `index.mdx`; `#quick-start` único ahí
- Sin critique previo; detector manual requerido (una sola pasada al final)

## Estado

Committed en rama `docs/landing-overdrive-reveal`: polish `fc743a3` +
overdrive `16d0ab3`. V2 (P0+P1) en cola, arranca con shape sin código.
