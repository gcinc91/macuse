# macuse

**Natural scrolling independiente para trackpad y ratón en Mac.**

macOS solo te deja activar o desactivar "natural scrolling" para todos los periféricos a la vez. Si tienes un Mac con trackpad y un ratón externo conectado, no puedes pedirle al sistema "quiero natural scrolling solo en el trackpad y el ratón al revés" (o viceversa).

`macuse` resuelve eso. Es una mini-app con dos toggles: uno para el trackpad y otro para el ratón. Cada uno controla el natural scrolling de ese periférico de forma totalmente independiente.

![cap](docs/screenshot.png)

## Lo que hace por dentro (en una frase)

Intercepta los eventos de scroll en el sistema y, según de qué periférico vengan, los invierte o no antes de pasarlos a las apps. **No toca el ajuste global de macOS**.

---

## Cómo instalarla (paso a paso para cualquiera)

Si nunca has compilado nada en tu vida, no pasa nada. Sigue estos pasos en orden y todo funcionará.

### 1. Abre la Terminal

Pulsa `Cmd + Espacio` para abrir Spotlight, escribe **Terminal** y pulsa Enter. Te aparecerá una ventana negra (o blanca) con texto. Ahí es donde vas a pegar los comandos.

> Para pegar en Terminal usa `Cmd + V`. Para ejecutar el comando que has pegado, pulsa Enter.

### 2. Instala Rust (la herramienta que compila la app)

Copia y pega este comando en la Terminal y pulsa Enter:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Te preguntará algo como "¿qué quieres hacer?". Pulsa Enter (acepta la opción por defecto, "1) Proceed with standard installation"). Tarda 1-2 minutos.

Cuando termine, ejecuta:

```bash
source "$HOME/.cargo/env"
```

> **¿Qué acabas de hacer?** Has instalado Rust, un compilador. Es como si te bajaras Photoshop para abrir un .psd: necesitas la herramienta para "abrir" el código y convertirlo en una app ejecutable.

### 3. Descarga el código

Copia, pega y ejecuta:

```bash
git clone https://github.com/gcinc91/macuse.git
cd macuse
```

> Si tu Mac te dice que no tiene `git` instalado, te aparecerá una ventana ofreciendo instalar las "Command Line Tools" — acéptala y espera a que termine. Después vuelve a ejecutar los comandos de arriba.

### 4. Compila y empaqueta la app

```bash
./make-app.sh
```

Esto tarda 1-3 minutos la primera vez. Cuando termine te dejará un archivo `macuse.app` en la carpeta. Es la app de verdad, como si la hubieras descargado de Internet.

### 5. Da permiso de Accesibilidad

Esto es **obligatorio** y solo se hace una vez. La app necesita permiso para "ver" los eventos de scroll del sistema y modificarlos.

1. Abre **Ajustes del Sistema** (la rueda dentada).
2. Ve a **Privacidad y Seguridad → Accesibilidad**.
3. Si en la lista aparece algo llamado `macuse` o `target/release/macuse` de antes, **bórralo** (selecciónalo y pulsa el botón **−**). Es importante hacerlo antes del siguiente paso.
4. Abre Finder, navega a la carpeta `macuse` que has descargado y **arrastra el archivo `macuse.app` a la lista de Accesibilidad**. (También puedes pulsar **+** y buscarla a mano.)
5. Asegúrate de que el switch al lado de `macuse` está **encendido (azul)**.

### 6. Lánzala

Vuelve a la Terminal (en la carpeta del proyecto) y ejecuta:

```bash
open macuse.app
```

O directamente desde Finder, haz doble clic en `macuse.app`.

Verás una ventana sencilla con dos toggles. **Listo**: cambia los toggles a tu gusto y prueba a hacer scroll con tu trackpad y/o ratón. La configuración se guarda automáticamente.

---

## Cómo se usa

La app es muy simple. Tiene dos secciones:

- **Trackpad**: el toggle de natural scrolling para el trackpad de tu portátil (o un Magic Trackpad).
- **Ratón**: el toggle de natural scrolling para cualquier ratón externo con rueda.

**¿Qué significa cada estado del toggle?**

| Toggle | Comportamiento |
|--------|----------------|
| **ON** (azul) | Natural scrolling: deslizas hacia arriba con el dedo / rueda → el contenido va hacia arriba (estilo iPhone, valor por defecto de macOS moderno). |
| **OFF** (gris) | Scrolling tradicional: deslizas hacia arriba → el contenido baja (estilo Windows, estilo Mac antes de 2011). |

El truco está en que ambos toggles son **independientes**. Lo más típico es:

- Trackpad **ON** + Ratón **OFF**: lo que quiere mucha gente. El trackpad va "natural" como en iOS y el ratón con rueda funciona como en Windows.

### "Iniciar al arrancar el Mac"

Hay un checkbox debajo. Si lo marcas, `macuse` se lanzará automáticamente cada vez que enciendas el Mac. Si lo desmarcas, se quita esa configuración.

---

## Si algo no va bien

### La app dice "Falta permiso de Accesibilidad" y pulsar Reintentar no hace nada

Lo más habitual: tienes una entrada antigua de `macuse` en la lista de Accesibilidad que está rota. Bórrala completamente, vuelve a arrastrar `macuse.app`, asegúrate de que está activada, y reabre la app.

### El scroll no se invierte aunque la app esté abierta y con permisos

Mira el log de diagnóstico:

```bash
tail -30 /tmp/macuse.log
```

Si ves `AX trusted (initial) = false`, sigues sin tener permiso (re-haz el paso 5).
Si ves `event tap habilitado en run loop` pero no aparece `evt#0` cuando haces scroll, hay otro tap antes que el nuestro. Cierra otras apps de scroll (Mos, Scroll Reverser, LinearMouse) y reabre macuse.

### Quiero quitar el auto-arranque

```bash
launchctl unload ~/Library/LaunchAgents/com.macuse.app.plist
rm ~/Library/LaunchAgents/com.macuse.app.plist
```

### Quiero desinstalar la app entera

```bash
# Detén la app
pkill -f "macuse.app/Contents/MacOS/macuse"

# Quita auto-arranque (si estaba activado)
launchctl unload ~/Library/LaunchAgents/com.macuse.app.plist 2>/dev/null
rm -f ~/Library/LaunchAgents/com.macuse.app.plist

# Borra la configuración guardada
rm -rf ~/Library/Application\ Support/macuse

# Borra la app
rm -rf ~/Documents/code/macuse  # o donde la hayas descargado
```

Y opcionalmente quita la entrada de `macuse` en Ajustes → Privacidad y Seguridad → Accesibilidad.

---

## Para curiosos: cómo funciona

`macuse` registra un **`CGEventTap`** a nivel `Session` que ve todos los eventos de scroll antes de que lleguen a las apps. En el callback:

1. Lee el campo `kCGScrollWheelEventIsContinuous` para saber si el evento viene de un trackpad (continuo, pixel-precise) o de un ratón con rueda (discreto, line-based). Es la heurística estándar usada por Scroll Reverser, Mos y LinearMouse.
2. Mira si el toggle del usuario para ese periférico coincide con el ajuste global de natural scrolling de macOS.
3. Si **coinciden**, deja el evento pasar tal cual.
4. Si **difieren**, invierte el signo de los deltas Y y X y reenvía el evento.

Así, sin tocar el ajuste global del Sistema, conseguimos comportamiento opuesto en cada periférico.

### Estructura del código

```
src/
├── main.rs              # bootstrap: carga config, comprueba permisos, arranca tap, abre ventana
├── ui.rs                # ventana Cocoa nativa con NSWindow + NSSwitch
├── log.rs               # logging a /tmp/macuse.log
├── config.rs            # serde JSON en ~/Library/Application Support/macuse/
├── permissions.rs       # AXIsProcessTrustedWithOptions
├── login_item.rs        # LaunchAgent plist en ~/Library/LaunchAgents/
├── system_pref.rs       # lectura de com.apple.swipescrolldirection
└── scroll/
    ├── state.rs         # ScrollState con AtomicBool compartido entre UI y tap callback
    ├── classify.rs      # is_trackpad() via kCGScrollWheelEventIsContinuous
    ├── transform.rs     # lógica de inversión XOR contra el ajuste global
    └── tap.rs           # CGEventTap + ciclo de vida + re-enable tras TapDisabled

make-app.sh              # compila y empaqueta como .app con bundle ID estable
Cargo.toml               # dependencias Rust (cocoa, objc, core-graphics, serde, ...)
```

### Por qué hay un `make-app.sh` en vez de solo `cargo build`

macOS guarda los permisos de Accesibilidad ligados a una identidad. Si das permiso a un binario suelto en `target/release/macuse`, al recompilar el hash cambia y macOS invalida el permiso. Empaquetando como `.app` con un **bundle ID** estable (`com.macuse.app` en `Info.plist`), el permiso sobrevive a los `cargo build` sucesivos.

### Compatibilidad

- macOS 11 (Big Sur) o superior.
- Apple Silicon (probado) e Intel (debería funcionar, no probado).

---

## Licencia

MIT.
