# ads-adapter-vault

`OsVault` implementa o contrato `ads_core::vault::Vault`. No Windows, guarda credenciais genéricas no Gerenciador de Credenciais do usuário atual, com persistência local entre sessões. Os alvos seguem `urn:ads:vault:<nome>:1` e vêm de `SecretName`. Cada entrada aceita de 1 a 2560 bytes. Guardar substitui o valor anterior, ler retorna `None` se a entrada estiver ausente e remover é idempotente. Em outros sistemas as operações retornam `Unsupported`.

A API nativa fica em um só módulo ([orçamento de unsafe](../../../docs/unsafe.md#ads-adapter-vault-cratesadaptersvaultsrcwindowsrs)). Veja a [ADR 0026](../../../docs/adr/0026-operating-system-vault.md) para a escolha do backend.

O teste real necessita de uma sessão Windows com cofre habilitado. A suíte padrão o ignora porque o cofre pode estar indisponível para a sessão do executor de CI. Para exercitar na sessão local:

```sh
cargo test -j 2 -p ads-adapter-vault credential_manager_round_trip -- --ignored
```

O teste gera um nome aleatório `ads-test-…` e instala uma guarda de limpeza após a primeira gravação bem-sucedida.
