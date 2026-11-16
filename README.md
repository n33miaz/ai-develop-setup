# ai-develop-setup

Codinome de uma plataforma local para delegar trabalho de desenvolvimento a agentes de IA: um daemon em Rust, uma casca Tauri e uma interface em React 19 + Vite + TypeScript. O nome comercial vem depois.

## Estado

Em construção, sem versão publicada. Já existem os contratos e os tipos gerados deles, o portão de ações destrutivas, o executor de arquivos, o host de terminais com o cliente dele no daemon, a abertura do banco local com a cifra por coluna e a interface com o cliente da API local (o núcleo ainda não serve essa API); o estado de cada módulo está em [docs/modules.md](docs/modules.md) e os riscos conhecidos em [docs/risks.md](docs/risks.md).

- Regras para quem altera o repo (pessoas e agentes): [AGENTS.md](AGENTS.md)
- Como contribuir e o fluxo de branches: [CONTRIBUTING.md](CONTRIBUTING.md)
- Segurança e relato de vulnerabilidade: [SECURITY.md](SECURITY.md)

## Licença

Código aberto à vista, direitos reservados: o código pode ser lido, estudado e avaliado, mas usar, copiar, modificar ou distribuir exige licença comercial escrita. Não é software livre nem open source. Texto completo, com a versão em português que prevalece, em [LICENSE](LICENSE).
