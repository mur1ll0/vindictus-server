# Add-on Vindictus para Blender 5.1

Importa e exporta **kits de modelo** do cliente 1.69: um modelo do jogo com tudo o que ele usa, nas pastas do jogo.

- `models/.../<nome>.mdl`, `.vvd`, `.dx90.vtx`, `.dx80.vtx` e `.sw.vtx`;
- `materials/<pasta>/<textura>.vmt` e as `.vtf` citadas;
- `kit.json` com o caminho do modelo.

Plano do editor de itens: [docs/future/item-editor.md](../../docs/future/item-editor.md).

## Instalar

1. Compacte a pasta `tools/blender/vindictus` num `.zip`, com a pasta dentro.
2. No Blender 5.1, abra *Edit → Preferences → Get Extensions → Install from Disk* e escolha o zip.

Para desenvolver, também dá para ligar a pasta em `%APPDATA%\Blender Foundation\Blender\5.1\extensions\user_default\vindictus`.

## Usar

- **Gerar um kit a partir do cliente** (o painel vai oferecer isso como botão):
  `py -3.13 scripts/client/model_kit.py --client-copy .cache/play-client --keystream .cache/play-client/hfs-keystream.bin models/player/fiona/fiona_inner_fringe.mdl <pasta do kit>`
- **Importar:** *File → Import → Vindictus Kit (kit.json)*. Escolha o que importar (malha, texturas, esqueleto), a escala e o destino.
  - O destino pode ser objetos novos, substituir a malha do objeto ativo ou somar à malha do objeto ativo.
  - Cada material vira um material do Blender com a textura base e o mapa de normais.
  - Os pesos viram grupos de vértices com o nome dos ossos, e a malha recebe um modificador Armature.
- **Editar:** mova vértices, pinte pesos ou edite as imagens importadas. Uma imagem cujos pixels mudarem é gravada de novo como VTF; as outras são copiadas sem mudança.
- **Exportar:** selecione as malhas importadas e use *File → Export → Vindictus Kit*. Escolha a pasta e o caminho do modelo novo (`models/player/fiona/fiona_inner_lab.mdl`).
  - O kit novo leva os materiais para `materials/models/<pasta do modelo>/<nome>/`.
  - Para virar item, o kit vai em `.cache/private-assets/<nome>`, com uma entrada em `scripts/content/item_additions.json` (exemplo: `fiona_inner_lab`).

## Como a exportação funciona

Não há `studiomdl` (compilador do Source SDK) na máquina, então a exportação **troca a malha** sobre o modelo-base do kit.

- **Do modelo-base:** o esqueleto, o cabeçalho, os anexos e as sequências.
- **O que é refeito:**
  - os vértices (VVD v4, até 3 pesos por vértice);
  - os triângulos (VTX v7, com paletas de até 53 ossos por faixa e pele por hardware);
  - o nome interno, a pasta de materiais e as contagens no MDL.
- **Níveis de detalhe:** todos repetem a geometria do nível 0.
- **Checksum:** o do modelo-base vale para o MDL, o VVD e os VTX.
- **Versões aceitas:** o motor 1.69 aceita MDL v44 (do jogo) e v48 (de clientes mais novos).

## Limites atuais

- Cada modelo precisa manter os mesmos materiais (malhas) do modelo-base: um slot de material para cada.
- Ossos novos não são criados; pesos em grupos sem osso correspondente são ignorados.
- Animações ainda não são importadas: ficam nos modelos de animação do personagem, e não no do item.
- As texturas são gravadas sem compressão (BGRA8888 com mipmaps), por isso ficam maiores que as DXT do jogo.

## Testes

```
"<Blender 5.1>/5.1/python/bin/python.exe" -m unittest discover -s tools/blender/vindictus/tests -t tools/blender/vindictus
```

Os formatos são testados com dados sintéticos. A ida e volta de um kit real só roda se `.cache/private-assets/kits/fiona_inner_fringe` existir; o conteúdo do jogo nunca é versionado.
