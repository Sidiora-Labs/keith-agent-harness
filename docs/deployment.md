# Deploy Keith

Keith's deployment unit is one stateful OCI image. It starts `agentd`, waits
for its Unix socket, starts `agent-web`, forwards termination to both, and fails
if either process exits. `/var/lib/keith` contains durable state and encrypted
credentials. Port `7341` serves the web UI, native integration API, and optional
OpenAI-compatible API.

This guide covers repository-supplied deployment paths. A rendered plan proves
manifest construction, not that an account, cluster, ingress, provider, backup,
or authenticated Keith journey works in a particular environment. Review the
[security policy](../SECURITY.md),
[configuration and data guide](features/configuration-profiles-and-data.md), and
[release qualification](release-qualification.md) before a public deployment.

## Required configuration

Every public deployment needs:

```sh
export KEITH_WEB_LOGIN_SECRET='a-long-random-login-secret'
export KEITH_PUBLIC_ORIGIN='https://keith.example.com'
export OPENAI_API_KEY='provider-secret' # or another supported provider variable
```

`KEITH_PUBLIC_ORIGIN` must exactly match the browser origin because it is part
of Keith's CSRF boundary. Set `KEITH_OPENAI_COMPAT_API_KEY` to enable `/v1`, and
`KEITH_PLATFORM_API_KEY` to enable `/platform/v1`. Use separate random values
for all three credentials.

At container boot, supported provider variables are imported into Keith's
encrypted credential store and removed from the child-process environment.
`GITHUB_TOKEN` is deliberately excluded from automatic import because CI and
container registries commonly inject it for unrelated purposes. To use the
GitHub Copilot provider, import its exchanged bearer token explicitly with
`agent-cli`.

## Docker Compose

```sh
cp .env.example .env
# Edit .env.
./keith up
./keith logs
./keith down
```

Compose builds the image locally, publishes port `7341`, mounts a named durable
volume, and bind-mounts `KEITH_WORKSPACE` at `/workspace`.

## Kubernetes and Helm

The chart at `deploy/kubernetes/helm/keith` creates one StatefulSet replica, a
20 GiB `ReadWriteOnce` claim, a service, health probes, a non-root security
context, and an optional ingress. A single replica is intentional: the current
durable store is locally owned and must not be concurrently mounted by several
Keith daemons.

Render and inspect the manifests without changing the cluster:

```sh
./keith deploy kubernetes --render --image ghcr.io/sidiora-labs/keith-agent:main
```

Deploy to the current `kubectl` context:

```sh
export KEITH_DEPLOY_APPROVED=YES
./keith deploy kubernetes --execute \
  --image ghcr.io/sidiora-labs/keith-agent:v0.1.0
```

The deployment command creates or updates an opaque `keith-secrets` Secret
from an owner-only temporary environment file, then removes that file. For a
GitOps installation, create the secret with your secret controller and invoke
Helm directly with `config.existingSecret` set to its name.

## Railway

`.railway/railway.ts` describes the image-backed service and its 20 GiB volume.
Link the checkout to the intended Railway project, export configuration values,
and review the plan:

```sh
railway link
./keith deploy railway
```

Apply only after reviewing the Railway plan:

```sh
export KEITH_DEPLOY_APPROVED=YES
./keith deploy railway --execute
```

The apply remains interactive. The tooling never passes Railway's unattended
destructive flags. Configure a Railway public or custom domain whose origin
matches `KEITH_PUBLIC_ORIGIN`.

Mount the persistent volume at `/var/lib/keith`, never at `/workspace`.
`/var/lib/keith` owns Keith's durable database, encrypted credentials, worker
registries, and recovery state; `/workspace` is the agent's working tree. The
container initializes either directory with the non-root Keith account when a
provider supplies a fresh root-owned mount, then permanently drops privileges
before importing credentials or starting any Keith process.

## Fly.io

```sh
./keith deploy fly --app my-keith --region iad
export KEITH_DEPLOY_APPROVED=YES
./keith deploy fly --execute --app my-keith --region iad
```

The execution path creates the app and one regional persistent volume when
missing, imports secrets over stdin, and performs a remote Docker build. It
keeps at least one machine running because Keith is a persistent agent runtime.

## Managed Kubernetes providers

The provider wrappers create a cluster only when it does not already exist,
select its kube context, and invoke the same Helm deployment:

```sh
./keith deploy digitalocean --cluster keith --region nyc1
./keith deploy azure --cluster keith --resource-group keith --location eastus
./keith deploy aws --cluster keith --region us-east-1
./keith deploy gcp --project PROJECT_ID --cluster keith --region us-central1
```

After reviewing the output, add `--execute` and set
`KEITH_DEPLOY_APPROVED=YES`. The required CLIs are `doctl`, `az`, `aws` plus
`eksctl`, or `gcloud`, respectively. All four paths then require `kubectl` and
`helm`.

Cluster creation is billable. The defaults use one capable node or GKE
Autopilot for a small installation; adjust size, count, region, storage class,
ingress, TLS, backups, and network policy for the intended workload.

## Production checklist

Before exposing Keith publicly:

1. Pin a version or image digest instead of `main`.
2. Terminate TLS at the provider load balancer or trusted ingress.
3. Restrict network access to the UI and enabled API surfaces.
4. Back up `/var/lib/keith` and test restoration.
5. Store all secrets in the provider's secret manager.
6. Monitor both `/login` and the container health result.
7. Verify the image's GitHub artifact attestation and SBOM.
8. Set the GHCR package to public before documenting anonymous pulls.
9. Confirm daemon restart with the persistent volume attached and restore a
   tested backup into an empty data root.
10. Exercise Web login and every enabled API or integration using its own
    credential; a healthy container alone is not an authenticated user journey.
11. Record which channel, connected-app, ACP, plugin, computer-use, and
    self-evolution paths were actually qualified in this deployment.
