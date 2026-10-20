# Bootstrap

This root creates the shared AWS resources the rest of the repository depends
on, including the S3 bucket (`state.stock-trek.com`) that stores the Terraform
state for both `bootstrap` and `live`.

## The chicken-and-egg problem

`bootstrap` is responsible for creating `state.stock-trek.com`, but it also
uses that same bucket as its own Terraform backend. On a brand new AWS account
the bucket does not exist yet, so `terraform init` cannot connect to the backend
and the configuration can never be applied.

The first apply therefore has to run against a local state file. Once the state
bucket exists, the local state is migrated into it and every later run uses the
S3 backend as normal.

## Fresh account

Prerequisites:

- AWS credentials with permission to create the resources in this root.
- The GitHub Actions OIDC provider
  (`token.actions.githubusercontent.com`) must already exist in the account,
  because this configuration reads it with a data source.

Run the helper script once:

```sh
./bootstrap/init.sh
```

The script:

1. Runs `terraform init -backend=false`, which initialises the providers and
   keeps state locally.
2. Applies only the state bucket module with the local backend.
3. Runs `terraform init -migrate-state -force-copy`, copying the local state
   into the bucket that was just created.
4. Applies the remaining bootstrap resources using the S3 backend.

After it completes, use the normal workflow:

```sh
terraform -chdir=bootstrap plan
terraform -chdir=bootstrap apply
```

The local state files produced during the first step are ignored by
`.gitignore` and must not be committed.

## Existing accounts

The bucket and its state already exist, so no special handling is required:
`terraform -chdir=bootstrap init/plan/apply` continues to work unchanged.
