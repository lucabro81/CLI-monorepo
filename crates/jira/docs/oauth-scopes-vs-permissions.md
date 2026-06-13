# OAuth scope vs Jira permission scheme

Due livelli di autorizzazione **completamente indipendenti**. Per ogni
chiamata API devono essere **entrambi** ok — basta che uno dei due manchi e
l'azione fallisce, con messaggi di errore che spesso non distinguono quale dei
due sia il colpevole.

## I due livelli

| | **OAuth scope** | **Jira permission scheme** |
|---|---|---|
| **Cosa è** | Lista di "capacità" che il *token* porta con sé (es. `read:jira-work`, `delete:issue:jira`) | Configurazione **per progetto** che dice quali ruoli/gruppi possono fare cosa (Browse, Create, Edit, Delete, Transition issues...) |
| **Dove si configura** | developer.atlassian.com → la tua OAuth app → Permissions (scope richiesti) | Jira → Project settings → **Permissions** (per ogni progetto, es. KAN) |
| **Chi lo concede** | Chi ha installato/autorizzato l'app sul sito (consenso 3LO una tantum, o admin del sito per `client_credentials`) | Project admin / Jira admin, assegnando ruoli (es. "Administrators", "Members") a utenti/gruppi nel permission scheme del progetto |
| **A cosa si applica** | Il token in generale, su tutto il sito | Un account specifico, su un progetto specifico |
| **Come si verifica** | Scope richiesti nell'authorization URL / consenso app | `GET /rest/api/3/mypermissions` (con o senza `projectKey`) |
| **Errore tipico se manca** | 403 generico, o l'endpoint nemmeno raggiungibile | `403 "You do not have permission to delete issues in this project."` |

## Schema del flusso

```
                ┌─────────────────────────────┐
 Richiesta -->  │  1. Il token ha lo scope?     │ --no--> 403 (scope insufficiente)
 "DELETE issue" │     (delete:issue:jira)       │
                └──────────────┬────────────────┘
                               │ sì
                               v
                ┌─────────────────────────────┐
                │  2. L'account del token ha    │ --no--> 403 "You do not have
                │     "Delete Issues" nel        │         permission to delete
                │     permission scheme di KAN?  │         issues in this project"
                └──────────────┬────────────────┘
                               │ sì
                               v
                          issue cancellata
```

## Nel nostro caso (jira-cli, progetto KAN)

| Componente | Cosa controlla | Stato attuale |
|---|---|---|
| **App OAuth** (developer.atlassian.com) | Quali scope il consenso ha concesso al token (`read:jira-user`, `read:jira-work`, `write:jira-work`, `delete:issue:jira`) | OK — scope presente, confermato dall'utente |
| **Service account `mercury`** (`client_credentials`) | È l'identità che esegue le chiamate; eredita gli scope dell'app | Token rigenerato con `auth login`, scope ok |
| **Permission scheme del progetto KAN** | Se l'account `mercury` può fare "Delete Issues" *in KAN* | **Manca** — 403 anche dopo re-login → blocco a livello 2 |

**Conclusione**: lo scope (livello 1) è ok, quindi il blocco è al livello 2 —
il permission scheme di KAN non assegna "Delete Issues" all'account
`mercury`. Va corretto in **KAN → Project settings → People/Permissions**,
non lato OAuth app. Non serve rifare login dopo questa modifica: il
permission scheme è valutato ad ogni richiesta, non è "cacheato" nel token.

### Soluzione effettiva (risolto)

Causa reale: `mercury` non era membro di **nessun project role** in KAN — e
"Delete Issues" nel permission scheme è assegnato a un project role (es.
"Administrators"), non a singoli utenti. Jira espone un **"permission
helper"** (cerca "why can't I..." nelle impostazioni permessi) che, dato
utente + issue + permesso, spiega esattamente quale condizione del permission
scheme non è soddisfatta — molto più diretto che leggere lo scheme a mano.

Fix: **KAN → Project settings → People** → aggiunto `mercury` al project role
che ha "Delete Issues" (Administrators). Risultato: `e2e_cleanup` ha
cancellato tutte le 50 issue orfane (KAN-44..93) senza bisogno di re-login.

## Nota su `jira doctor`

`doctor`'s `permissions` check chiama `mypermissions` **senza** contesto di
progetto — quindi riflette solo il livello 1 (scope), non il livello 2
(permission scheme per-progetto). `DELETE_ISSUES: false` in `doctor` può
quindi significare "scope assente" **oppure** "scope presente ma nessun
progetto lo concede globalmente" — non distingue i due casi. Vedi BACKLOG
`DOCTOR-1` per la discussione su questo limite.
