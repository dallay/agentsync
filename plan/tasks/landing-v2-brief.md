# Brief V2 landing — homepage producto (shape, sin código)

Estado: pendiente de confirmación explícita. Shape no escribe código.

## 1. Job y audiencia

Dev con 2+ asistentes AI que cae en la home de docs. Modo Persuade:
en <60s debe entender el mecanismo (one→many vía symlinks) y tener el
comando de install copiado. Éxito = time-to-first-command en segundos.

## 2. Outcome y prueba

Primario: `npx @dallay/agentsync init` copiado/ejecutado → Getting Started.
Evidencia real (CLI 1.50.x, verificada en sandbox):

- apply: `✔ Linked: <dest> -> <source>` por agente,
  `✔ Updated .gitignore with N managed entries`, `✔ Sync complete`
- status: `✔ OK: <dest> -> <source>`, `Status: All good`
- 10 agentes nativos MCP, 4 sync types, `plugins.lock.toml` + `allowed_mcp`

Prohibido inventar output, testimonios, benchmarks o afiliaciones.

## 3. Dirección elegida

Mundo intacto: El Terminal Luminoso (tokens `--as-*`, Geist, dark-first).
Tesis estructural: el diagrama del mecanismo posee el hero (mascota +
producto, no mascota en lugar de producto); compatibilidad como evidencia;
secuencia Define→Apply→Synced como lenguaje de marca reutilizable.
Momento focal: diagrama hero + comando install. El revelado por scroll
(overdrive A, ya en `main` de la rama) se extiende a las secciones nuevas.

Copy confirmado: H1 idea "Configure once…" (el usuario aprobó la dirección;
"Sync AI Agents" es ambiguo: se sincroniza la configuración, no los agentes).

## 4. Alcance P0+P1 (confirmado por el usuario)

P0: hero con diagrama + CTA install con Copy; sección "How it works" 01–03.
P1: bento 3 grandes (single source / symlinks nativos / compatibilidad) +
4 pequeñas (MCP, skills, gitignore, cross-platform); emoji → iconos SVG de
trazo consistente; rail "Works with the tools you already use"; fix mobile
(copy antes que visual, robot reducido).
Terminal narrativa init→apply→status con transcript real (elegida en Q3).
Fuera: sección Use cases (opcional), vídeo, logos a color, tocar Starlight,
paleta, footer (solo revisar nomenclatura), DESIGN.md.

## 5. Estados y rangos

Rail: 6–10 nombres; terminal: 8–14 líneas + CTA a Docs; bento 3+4.
Móvil: headline → sub → install → compatibilidad → visual pequeño.
Reduced-motion: todo estático y visible (restaura opacidad).
Claro + oscuro: contraste AA en ambos (hover cyan en dark, violeta en light).

## 6. Interacción y layout (intención, no CSS)

Desktop: hero 2 col (copy + diagrama), rail, 3 pasos, bento asimétrico,
terminal, CTA final. Números 01–03 ganados (la secuencia ES la información).
Botón Copy con estado success; foco `:focus-visible` siempre; targets 44px;
tabs de Quick Start de referencia quedan dentro de Docs, no en la landing.

## 7. Restricciones y decisiones abiertas del constructor

Astro splash + `base: /agentsync`: todo link interno con
BaseLink/withBase (el bug de 404 ya se fixed una vez). Mono solo para código.
Sin nuevos roles de color, sin sombras en reposo, sin `!important`.
Decisiones abiertas: set exacto de iconos SVG (trazo único); implementación
del diagrama (SVG geométrico nítido, no ilustración sketch — el floor lo
prohíbe); runner del comando hero (propuesta: npx, el más universal).

## 8. Reglas de aceptación (añadidas en la confirmación)

1. Hero: un visitante nuevo debe identificar la fuente canónica, entender
   que AgentSync materializa symlinks nativos para múltiples agentes y
   copiar el comando de install sin hacer scroll.
2. La evidencia de producto tiene prioridad sobre la densidad decorativa.
   Si la mascota, la animación, el chrome de la terminal, las marcas de
   compatibilidad o los efectos visuales compiten con el diagrama del
   mecanismo o el comando de install, se reducen o eliminan.

Estado: CONFIRMADO e IMPLEMENTADO — commit `d0e67c5` en rama
`docs/landing-overdrive-reveal` (tras `fc743a3` polish + `16d0ab3`
overdrive). Verificado: build 17 páginas, detector sin hits nuevos,
navegador desktop+móvil+scroll+reduced-motion, 0 errores de consola.
