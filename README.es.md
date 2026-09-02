<h1 align="center">Keith</h1>

<p align="center">
  <strong>El primer agente que evoluciona de verdad: no mediante trucos de prompt, sino modificando, probando y promoviendo de forma segura cambios en su propio harness.</strong>
</p>

<p align="center">
  Keith convierte la experiencia real en cambios a la maquinaria que define cómo
  razona, elige herramientas, gestiona el contexto y termina el trabajo — no en
  otra nota más en un prompt. Cada nueva versión se construye aislada, se prueba
  contra el Keith actual, y solo se adopta si resulta mejor sin cruzar tus
  límites de seguridad.
</p>

<p align="center">
  <a href="https://github.com/Sidiora-Labs/keith-agent/actions/workflows/ci.yml"><img src="https://github.com/Sidiora-Labs/keith-agent/actions/workflows/ci.yml/badge.svg" alt="Estado de CI"></a>
  <a href="https://github.com/Sidiora-Labs/keith-agent/releases/latest"><img src="https://img.shields.io/github/v/release/Sidiora-Labs/keith-agent?display_name=tag" alt="Última versión"></a>
  <a href="https://github.com/Sidiora-Labs/keith-agent/stargazers"><img src="https://img.shields.io/github/stars/Sidiora-Labs/keith-agent?style=flat" alt="Estrellas en GitHub"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-green" alt="Licencia: Apache-2.0"></a>
  <a href="https://github.com/Sidiora-Labs/keith-agent/pkgs/container/keith-agent"><img src="https://img.shields.io/badge/container-GHCR-blue" alt="Imagen GHCR"></a>
</p>

<p align="center">
  <a href="docs/installation.md">Empezar</a> ·
  <a href="docs/deployment.md">Desplegar</a> ·
  <a href="docs/crate-guide.md">Arquitectura</a> ·
  <a href="CONTRIBUTING.md">Contribuir</a> ·
  <a href="SECURITY.md">Seguridad</a>
</p>

<p align="center">
  <img src="docs/assets/keith-harness-repair.png" alt="Keith probando una reparación de su propio harness de forma aislada" width="1100">
</p>

<p align="center"><sub>Aprende del trabajo real. Construye un mejor harness. Demuestra la mejora antes de llevarla a producción.</sub></p>

> [!IMPORTANT]
> Keith es software previo al lanzamiento. El núcleo del sistema funciona, pero
> las interfaces, el almacenamiento y el empaquetado aún pueden cambiar antes
> de la 1.0.

## Pruébalo

La vía más corta es Docker:

```bash
cp .env.example .env
# Define KEITH_WEB_LOGIN_SECRET y la clave de un proveedor de modelos en .env.
docker compose up --build
```

Abre <http://localhost:7341>. Keith guarda su estado en el volumen `keith-data`
y puede trabajar dentro del checkout actual en `/workspace`.

¿Prefieres desarrollar desde el código fuente?

```bash
./keith doctor
./keith setup
./keith dev
```

Consulta la [guía de instalación](docs/installation.md) para ver TUI, configuración
de proveedores, actualizaciones, copias de seguridad y gestión del servicio.

## Cómo evoluciona Keith

Cada ejecución le da a Keith mucho más que una transcripción. Produce evidencias
sobre cómo funcionó todo el agente: el camino de razonamiento, las elecciones de
herramientas, el uso de contexto, la latencia, el coste, la recuperación y el
resultado final.

Un fallo duro puede revelar una oportunidad de evolución, pero también una llamada
a herramienta derrochadora, una recuperación lenta o un resultado que debería
haber sido mejor.

Keith convierte la mejor oportunidad en una hipótesis comprobable, construye
varios harnesses candidatos lejos del sistema en producción y los compara con la
versión actual sobre un trabajo que quien propuso la idea no pudo ver. Un candidato
solo avanza cuando produce una mejora medible sin introducir regresiones.

```text
experiencia → oportunidad → hipótesis comprobable → harnesses candidatos
           → evaluación retenida → canary → observar → mantener o revertir
```

Esa es la apuesta central detrás de Keith: un agente no solo debe hacer trabajo,
sino que debe tener una forma segura e inspeccionable de mejorar al hacerlo.

### Corrígelo mientras trabaja

Keith Computer es un escritorio visible y aislado. Observa la ejecución en vivo,
toma el teclado y el puntero con una sola acción, corrige el problema y devuelve
el control. Una concesión exclusiva de control evita que tú y Keith peleéis por
la misma pantalla.

### Enséñale el trabajo, no otro prompt

Cuando le demuestras una tarea, Keith registra la estructura útil que hay detrás:
estado de pantalla, acciones de teclado y puntero, objetivos de UI, contexto de
la aplicación, tiempos, narración, archivos, actividad del portapapeles y cada
cambio de control. Convierte esas evidencias en un **TaskRecipe** editable con
entradas, puntos de control, aprobaciones, pasos de recuperación, versiones y
rollback.

No le estás dando a Keith una grabación de pantalla. Le estás mostrando un
trozo de trabajo que puede reproducir y mejorar.

### Deja que repare el harness, no las reglas

La autoreparación solo es útil si el candidato no puede mover los postes de la
portería. Los candidatos de reparación de Keith no pueden editar al juez, los
casos de prueba retenidos, las credenciales, lo que Keith puede recordar o
revelar, tus reglas de aprobación, las comprobaciones de release, la puerta de
promoción ni la ruta de rollback.

Elige hasta dónde puede llegar Keith:

- **Solo aconsejar**   explica la reparación propuesta y espera.
- **Prueba en sombra**   construye y prueba candidatos, pero no promueve ninguno.
- **Reparación autónoma**   canary, observar y revertir dentro de los límites que definas.

En todos los modos, las reglas protegidas quedan fuera del alcance del candidato.

### Un Keith, no una carpeta de bots

La Web UI, la TUI, la API compatible con OpenAI, la API nativa, los clientes
ACP, los canales de mensajería, las apps conectadas y el equipo informático
llegan todos al mismo agente propiedad del demonio. Los workers especialistas
pueden investigar, programar o manejar herramientas entre bastidores sin
convertir el producto en un panel lleno de personalidades que gestionar.

Las sesiones sobreviven a desconexiones de clientes. Compromisos, esperas,
trabajos programados, objetivos y ejecuciones activas pueden recuperarse tras
un reinicio. Empieza en la terminal, sigue desde Slack y termina en el navegador
sin crear tres asistentes inconexos.

## Qué puedes hacer con Keith

| Si quieres… | Keith puede… |
| --- | --- |
| Delegar una tarea de navegador o escritorio | Trabajar en un equipo con o sin pantalla mientras miras, pausas o tomas el control |
| Enseñar un flujo de trabajo repetible | Convertir una demostración en vivo en un TaskRecipe editable y reproducible |
| Dejar de repetir el mismo fallo del agente | Diagnosticar el harness, probar reparaciones competidoras, hacer canary del ganador y revertir |
| Usar tus propios modelos | Enrutar perfiles a través de OpenAI, Anthropic, OpenRouter, Ollama u otro proveedor compatible |
| Llegar al mismo agente desde cualquier lugar | Servir clientes Web, TUI, ACP, API, Discord, Slack, Telegram, WhatsApp, Teams, Google Chat, correo y Matrix |
| Conectar servicios reales | Usar apps conectadas con aprobación, Composio, servidores MCP y plugins WASI con capacidades acotadas |
| Construir sobre Keith | Usar la API `/v1` compatible con OpenAI o la API `/platform/v1` nativa y tipada |
| Ejecutarlo en tu infraestructura | Desplegar con Docker, Kubernetes, Railway, Fly.io, DigitalOcean, Azure, AWS o Google Cloud |

## Un único runtime, muchas formas de entrar

```text
Web · TUI · OpenAI API · Platform API · ACP · Canales
                         │
                      agentd
              sesiones · política · recuperación
                         │
                 workers del agente en concesión
                         │
       modelos · herramientas · plugins · CUA · apps conectadas
```

`agentd` es la fuente de verdad. Los clientes se limitan a renderizar sus sesiones
y su ciclo de vida, en lugar de inventar su propio estado. Los workers ejecutan
turnos bajo concesión, y los crates de dominio mantienen la política separada de
los adaptadores externos.

Lee la [guía de crates](docs/crate-guide.md) y las
[fronteras de dependencias](docs/architecture/dependency-boundaries.md) para
ver el mapa completo del sistema.

## APIs y extensiones

Keith expone dos superficies HTTP:

- **`/v1` compatible con OpenAI** para SDKs y herramientas existentes como
  Open WebUI.
- **`/platform/v1` nativo** para clientes de confianza que necesitan sesiones,
  ciclo de vida, aprobaciones, artefactos y eventos en vivo de Keith.

Las extensiones pueden ejecutarse como componentes WASI con capacidades acotadas,
servidores MCP, skills o apps conectadas con aprobación. Los clientes ACP pueden
conectarse a través del servidor stdio incluido. Consulta la
[compatibilidad con OpenAI](docs/openai-compatibility.md) y la
[integración con la plataforma](docs/platform-integration.md).

## Seguridad

> [!WARNING]
> Keith puede ejecutar comandos, modificar archivos, controlar un navegador y
> llamar a servicios externos con la autoridad que le concedas. Usa un espacio
> de trabajo que puedas inspeccionar y restaurar. Trata la salida del modelo,
> los mensajes de los canales, las páginas obtenidas, los plugins, las skills,
> los servidores MCP y los candidatos de reparación como no confiables.

Mantén la Web UI y las APIs en loopback a menos que añadas TLS, autenticación
fuerte y una política de red explícita. Usa secretos diferentes para el login
web, las APIs, los proveedores de modelos y la firma de releases. Nunca publiques
credenciales ni trazas sin censurar en un issue público.

Reporta las vulnerabilidades en privado a través del
[formulario de security advisory](https://github.com/Sidiora-Labs/keith-agent/security/advisories/new)
de GitHub. Lee [SECURITY.md](SECURITY.md) para conocer el modelo de confianza,
el alcance y las reglas de reporte.

## Despliegue

Keith se distribuye como una única imagen OCI con estado, con vías compatibles
para Docker Compose, Kubernetes con Helm, Railway, Fly.io, DigitalOcean
Kubernetes, Azure Kubernetes Service, Amazon EKS y Google Kubernetes Engine
Autopilot.

```bash
./keith deploy kubernetes --render
./keith deploy railway
./keith deploy fly --app my-keith
./keith deploy aws --cluster keith --region us-east-1
```

Los comandos de nube imprimen un plan por defecto. Un despliegue solo cambia la
infraestructura cuando pasas `--execute` y defines `KEITH_DEPLOY_APPROVED=YES`.
Lee la [guía de despliegue](docs/deployment.md) completa antes de exponer Keith
fuera del host.

## Desarrollar y extender

El comando `./keith` es el punto de entrada del contribuidor para setup, servicios
locales, comprobaciones, tests, builds de release, imágenes de contenedor,
scaffolding y planes de despliegue. Los artefactos de compilación de Rust se
mantienen fuera del checkout.

```bash
./keith check
./keith test
./keith image keith-agent:dev
./keith scaffold plugin my-plugin
./keith scaffold skill my-skill
```

Se requieren Rust 1.93, Node.js 22.22, Corepack y Git. Antes de abrir un pull
request, lee [CONTRIBUTING.md](CONTRIBUTING.md) e indica los comandos y las
rutas de usuario reales que realmente ejecutaste.

## Documentación

| Objetivo | Empieza aquí |
| --- | --- |
| Instalar, configurar un proveedor o ejecutar la TUI | [Instalación y ciclo de vida](docs/installation.md) |
| Ejecutar con Docker o un proveedor cloud | [Guía de despliegue](docs/deployment.md) |
| Conectar un SDK de OpenAI u Open WebUI | [Compatibilidad con OpenAI](docs/openai-compatibility.md) |
| Integrar un cliente nativo de confianza | [Integración con la plataforma](docs/platform-integration.md) |
| Entender el workspace | [Guía de crates](docs/crate-guide.md) |
| Calificar un release | [Cualificación de release](docs/release-qualification.md) |
| Pedir ayuda o reportar un problema | [Soporte](SUPPORT.md) |

## Comunidad

- Haz preguntas y comparte ideas en
  [GitHub Discussions](https://github.com/Sidiora-Labs/keith-agent/discussions).
- Reporta bugs reproducibles a través de los
  [formularios de issue](https://github.com/Sidiora-Labs/keith-agent/issues/new/choose).
- Reporta los problemas de seguridad en privado, nunca en un issue público.
- Sigue el [Código de Conducta](CODE_OF_CONDUCT.md) en todos los espacios del
  proyecto.

## Licencia

Keith está disponible bajo la [Apache License 2.0](LICENSE).
