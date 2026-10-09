 1. Dependencias (pacman)

 ```bash
   sudo pacman -S wtype wl-clipboard libnotify
   # pipewire-alsa ya suele estar; si no: sudo pacman -S pipewire-alsa
 ```

 2. Instalar voxtype (AUR)

 Con helper:

 ```bash
   paru -S voxtype-bin        # o: yay -S voxtype-bin
 ```

 Sin helper (manual):

 ```bash
   sudo pacman -S base-devel git   # si te falta
   git clone https://aur.archlinux.org/voxtype-bin.git
   cd voxtype-bin && makepkg -si
 ```

 3. Descargar modelo multilingüe (español)

 ```bash
   voxtype setup model
 ```

 Elegir opción [5] small (466 MB, multi). ⚠️ NO elegir modelos .en (solo inglés).

 4. Configurar idioma español

 ```bash
   sed -i 's/^language = "en"/language = "es"/' ~/.config/voxtype/config.toml
   # verificar:
   grep "^language" ~/.config/voxtype/config.toml   # debe decir: language = "es"
 ```

 5. Arrancar el daemon

 ```bash
   systemctl --user enable --now voxtype
   systemctl --user status voxtype   # verificar que dice "active (running)"
 ```

 6. Test de transcripción

 ```bash
   arecord -d 5 -f S16_LE -r 16000 /tmp/test.wav   # hablar 5 segundos
   voxtype transcribe /tmp/test.wav                 # debe devolver el texto en español
 ```

 7. Test del modo daemon (el que usaremos en WeztCode)

 ```bash
   voxtype record start --file=/tmp/stt.txt --no-osd   # empieza a grabar
   # ... hablar ...
   voxtype record stop                                 # para y transcribe
   cat /tmp/stt.txt                                     # el texto debería estar acá
 ```

 ────────────────────────────────────────────────────────────────────────────────

 Notas:
 - Grupo input: NO hace falta (solo es para hotkeys evdev; nosotros controlamos por CLI)
 - Si el audio no captura: verificar mic con arecord -L y en config [audio] device = "default"
 - Todo lo demás (hotkey ScrollLock, output mode type) no lo tocamos — WeztCode va a manejar la grabación por su cuenta vía voxtype record start/stop

 Cuando la tengas configurada en la laptop, avisame y arrancamos con la integración del mic en el chat.
