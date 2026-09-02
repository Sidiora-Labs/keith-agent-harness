<h1 align="center">Keith</h1>

<p align="center">
  <strong>O primeiro agente que realmente evolui — não por truques de prompt, mas modificando, testando e promovendo com segurança mudanças no seu próprio harness.</strong>
</p>

<p align="center">
  Keith transforma experiência real em mudanças na máquina que define como ele
  raciocina, escolhe ferramentas, gerencia contexto e entrega trabalho — não em
  mais uma nota em um prompt. Cada nova versão é construída isolada, testada
  contra o Keith atual, e só é adotada quando se mostra melhor sem ultrapassar
  os seus limites de segurança.
</p>

<p align="center">
  <a href="https://github.com/Sidiora-Labs/keith-agent/actions/workflows/ci.yml"><img src="https://github.com/Sidiora-Labs/keith-agent/actions/workflows/ci.yml/badge.svg" alt="Status do CI"></a>
  <a href="https://github.com/Sidiora-Labs/keith-agent/releases/latest"><img src="https://img.shields.io/github/v/release/Sidiora-Labs/keith-agent?display_name=tag" alt="Última versão"></a>
  <a href="https://github.com/Sidiora-Labs/keith-agent/stargazers"><img src="https://img.shields.io/github/stars/Sidiora-Labs/keith-agent?style=flat" alt="Estrelas no GitHub"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-green" alt="Licença: Apache-2.0"></a>
  <a href="https://github.com/Sidiora-Labs/keith-agent/pkgs/container/keith-agent"><img src="https://img.shields.io/badge/container-GHCR-blue" alt="Imagem GHCR"></a>
</p>

<p align="center">
  <a href="docs/installation.md">Começar</a> ·
  <a href="docs/deployment.md">Implantar</a> ·
  <a href="docs/crate-guide.md">Arquitetura</a> ·
  <a href="CONTRIBUTING.md">Contribuir</a> ·
  <a href="SECURITY.md">Segurança</a>
</p>

<p align="center">
  <img src="docs/assets/keith-harness-repair.png" alt="Keith testando um reparo no próprio harness de forma isolada" width="1100">
</p>

<p align="center"><sub>Aprenda com trabalho real. Construa um harness melhor. Prove a melhoria antes de colocá-lo em produção.</sub></p>

> [!IMPORTANT]
> Keith é um software em pré-lançamento. O núcleo do sistema funciona, mas
> interfaces, armazenamento e empacotamento ainda podem mudar até a 1.0.

## Experimente

O caminho mais curto é o Docker:

```bash
cp .env.example .env
# Defina KEITH_WEB_LOGIN_SECRET e a chave de um provedor de modelos em .env.
docker compose up --build
```

Abra <http://localhost:7341>. Keith mantém o estado no volume `keith-data` e
pode trabalhar dentro do checkout atual em `/workspace`.

Desenvolvendo a partir do código-fonte?

```bash
./keith doctor
./keith setup
./keith dev
```

Veja o [guia de instalação](docs/installation.md) para a TUI, configuração de
provedores, upgrades, backups e gestão do serviço.

## Como Keith evolui

Cada execução dá a Keith mais do que uma transcrição. Ela produz evidências
sobre como o agente inteiro se saiu: o caminho de raciocínio, escolhas de
ferramenta, uso de contexto, latência, custo, recuperação e resultado final.

Uma falha grave pode expor uma oportunidade de evolução, mas o mesmo vale para
uma chamada de ferramenta desperddiçadora, uma recuperação lenta ou um resultado
que poderia ter sido melhor.

Keith transforma a melhor oportunidade em uma hipótese testável, constrói vários
harnesses candidatos longe do sistema em produção e os compara com a versão
atual em um trabalho que o proponente não pôde ver. Um candidato só avança
quando produz uma melhoria mensurável sem introduzir regressões.

```text
experiência → oportunidade → hipótese testável → harnesses candidatos
           → avaliação retida → canário → observar → manter ou reverter
```

Essa é a aposta central por trás de Keith: um agente não deve apenas fazer
trabalho. Ele deve ter uma forma segura e inspecionável de ficar melhor em
fazê-lo.

### Corrija enquanto ele trabalha

Keith Computer é um desktop visível e isolado. Acompanhe a execução ao vivo,
tome o teclado e o ponteiro em uma ação, corrija o problema e devolva o
controle. Uma concessão exclusiva de controle evita que você e Keith disputem
a mesma tela.

### Ensine o trabalho, não outro prompt

Quando você demonstra uma tarefa, Keith registra a estrutura útil por trás dela:
estado da tela, ações de teclado e ponteiro, alvos de UI, contexto do aplicativo,
tempos, narração, arquivos, atividade da área de transferência e cada troca de
controle. Ele transforma essas evidências em um **TaskRecipe** editável, com
entradas, checkpoints, aprovações, etapas de recuperação, versões e rollback.

Você não está apenas dando a Keith uma gravação de tela. Está mostrando a ele
um pedaço de trabalho que ele pode reproduzir e melhorar.

### Deixe-o consertar o harness, não as regras

O auto-reparo só é útil se o candidato não puder mover as traves. Os candidatos
de reparo do Keith não podem editar o juiz, os casos de teste retidos, as
credenciais, o que Keith pode lembrar ou revelar, suas regras de aprovação, as
verificações de release, o portão de promoção nem o caminho de rollback.

Escolha até onde Keith pode ir:

- **Apenas aconselhar**   explica o reparo proposto e espera.
- **Teste em sombra**   constrói e testa candidatos, mas não promove nenhum.
- **Reparo autônomo**   canário, observar e reverter dentro dos limites que você definir.

Em todos os modos, as regras protegidas ficam fora do alcance do candidato.

### Um Keith, não uma pasta de bots

A Web UI, a TUI, a API compatível com OpenAI, a API nativa, os clientes ACP,
os canais de mensageria, os apps conectados e o computador alcançam o mesmo
agente de propriedade do daemon. Workers especialistas podem pesquisar, codar
ou operar ferramentas nos bastidores sem transformar o produto em um painel
repleto de personalidades a gerenciar.

As sessões sobrevivem a desconexões dos clientes. Compromissos, esperas,
trabalhos agendados, metas e execuções ativas podem se recuperar após um
reinício. Comece no terminal, confira pelo Slack e finalize no navegador, sem
criar três assistentes sem relação.

## O que você pode fazer com Keith

| Você quer… | Keith pode… |
| --- | --- |
| Delegar uma tarefa no navegador ou no desktop | Trabalhar em um computador com ou sem tela enquanto você observa, pausa ou assume o controle |
| Ensinar um fluxo de trabalho repetível | Transformar uma demonstração ao vivo em um TaskRecipe editável e reproduzível |
| Parar de repetir a mesma falha do agente | Diagnosticar o harness, testar reparos concorrentes, fazer canário do vencedor e reverter |
| Usar seus próprios modelos | Rotear perfis via OpenAI, Anthropic, OpenRouter, Ollama ou outro provedor compatível |
| Acessar o mesmo agente de qualquer lugar | Atender clientes Web, TUI, ACP, API, Discord, Slack, Telegram, WhatsApp, Teams, Google Chat, e-mail e Matrix |
| Conectar serviços reais | Usar apps conectados com aprovação, Composio, servidores MCP e plugins WASI com capacidades restritas |
| Construir em cima do Keith | Usar a API `/v1` compatível com OpenAI ou a API `/platform/v1` nativa e tipada |
| Rodar na sua infraestrutura | Implantar com Docker, Kubernetes, Railway, Fly.io, DigitalOcean, Azure, AWS ou Google Cloud |

## Um runtime, muitas formas de entrar

```text
Web · TUI · OpenAI API · Platform API · ACP · Canais
                         │
                      agentd
              sessões · política · recuperação
                         │
                 workers do agente em concessão
                         │
       modelos · ferramentas · plugins · CUA · apps conectados
```

`agentd` é a fonte da verdade. Os clientes apenas renderizam suas sessões e ciclo
de vida, em vez de inventar seu próprio estado. Os workers executam turnos sob
concessão, e os crates de domínio mantêm a política separada dos adaptadores
externos.

Leia o [guia de crates](docs/crate-guide.md) e as
[fronteiras de dependência](docs/architecture/dependency-boundaries.md) para o
mapa completo do sistema.

## APIs e extensões

Keith expõe duas superfícies HTTP:

- **`/v1` compatível com OpenAI** para SDKs e ferramentas existentes como
  Open WebUI.
- **`/platform/v1` nativo** para clientes confiáveis que precisam de sessões,
  ciclo de vida, aprovações, artefatos e eventos ao vivo do Keith.

Extensões podem rodar como componentes WASI com capacidades restritas, servidores
MCP, skills ou apps conectados com aprovação. Clientes ACP podem se conectar
pelo servidor stdio embutido. Veja a
[compatibilidade com OpenAI](docs/openai-compatibility.md) e a
[integração com a plataforma](docs/platform-integration.md).

## Segurança

> [!WARNING]
> Keith pode executar comandos, alterar arquivos, controlar um navegador e chamar
> serviços externos com a autoridade que você lhe conceder. Use um workspace que
> você possa inspecionar e restaurar. Trate a saída do modelo, mensagens de
> canais, páginas obtidas, plugins, skills, servidores MCP e candidatos de
> reparo como não confiáveis.

Mantenha a Web UI e as APIs em loopback a menos que você adicione TLS,
autenticação forte e uma política de rede explícita. Use segredos diferentes
para login web, APIs, provedores de modelos e assinatura de releases. Nunca
poste credenciais ou traces sem editar em uma issue pública.

Reporte vulnerabilidades em privado pelo
[formulário de security advisory](https://github.com/Sidiora-Labs/keith-agent/security/advisories/new)
do GitHub. Leia o [SECURITY.md](SECURITY.md) para o modelo de confiança, escopo
e regras de reporte.

## Implantação

Keith é distribuído como uma única imagem OCI com estado, com caminhos suportados
para Docker Compose, Kubernetes com Helm, Railway, Fly.io, DigitalOcean
Kubernetes, Azure Kubernetes Service, Amazon EKS e Google Kubernetes Engine
Autopilot.

```bash
./keith deploy kubernetes --render
./keith deploy railway
./keith deploy fly --app my-keith
./keith deploy aws --cluster keith --region us-east-1
```

Os comandos de nuvem imprimem um plano por padrão. Uma implantação só altera a
infraestrutura quando você passa `--execute` e define `KEITH_DEPLOY_APPROVED=YES`.
Leia o [guia de implantação](docs/deployment.md) completo antes de expor Keith
fora do host.

## Desenvolver e estender

O comando `./keith` é o ponto de entrada do contribuidor para setup, serviços
locais, checagens, testes, builds de release, imagens de contêiner, scaffolding
e planos de implantação. Os artefatos de build em Rust ficam fora do checkout.

```bash
./keith check
./keith test
./keith image keith-agent:dev
./keith scaffold plugin my-plugin
./keith scaffold skill my-skill
```

São exigidos Rust 1.93, Node.js 22.22, Corepack e Git. Antes de abrir um pull
request, leia o [CONTRIBUTING.md](CONTRIBUTING.md) e relate os comandos e os
caminhos reais de usuário que você de fato executou.

## Documentação

| Objetivo | Comece por aqui |
| --- | --- |
| Instalar, configurar um provedor ou rodar a TUI | [Instalação e ciclo de vida](docs/installation.md) |
| Rodar com Docker ou um provedor de nuvem | [Guia de implantação](docs/deployment.md) |
| Conectar um SDK OpenAI ou Open WebUI | [Compatibilidade com OpenAI](docs/openai-compatibility.md) |
| Integrar um cliente nativo confiável | [Integração com a plataforma](docs/platform-integration.md) |
| Entender o workspace | [Guia de crates](docs/crate-guide.md) |
| Qualificar um release | [Qualificação de release](docs/release-qualification.md) |
| Pedir ajuda ou relatar um problema | [Suporte](SUPPORT.md) |

## Comunidade

- Faça perguntas e compartilhe ideias nas
  [Discussões do GitHub](https://github.com/Sidiora-Labs/keith-agent/discussions).
- Reporte bugs reproduzíveis pelos
  [formulários de issue](https://github.com/Sidiora-Labs/keith-agent/issues/new/choose).
- Reporte problemas de segurança em privado, nunca em uma issue pública.
- Siga o [Código de Conduta](CODE_OF_CONDUCT.md) em todos os espaços do projeto.

## Licença

Keith está disponível sob a [Apache License 2.0](LICENSE).
