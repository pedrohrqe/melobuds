"""Mantém a conexão BLE com o fone e expõe o estado como sinais Qt."""

from __future__ import annotations

import asyncio
import logging
import time
from dataclasses import replace

from bleak.exc import BleakBluetoothNotAvailableError
from PySide6.QtCore import QObject, QSettings, Signal

from .. import classic
from .. import protocol as p
from ..device import HT05, DeviceState, find_first_with_adv

log = logging.getLogger(__name__)

HOLD_S = 20         # tempo em que a tela mantém o estado depois de uma queda do BLE
AUDIO_GRACE_S = 1.5   # pausa curta: evita pedir o áudio no mesmo instante em que o fone chama o computador
PROBE_IDLE_S = 15      # intervalo da sonda do elo de controle
PROBE_TIMEOUT_S = 4    # espera pela resposta da sonda


def _in_case(battery) -> bool:
    """Os dois fones estão na caixa: cada um carrega ou não aparece, e ao menos um carrega."""
    if battery is None:
        return False
    buds = (battery.left, battery.right)
    return any(b.charging for b in buds) and all(b.charging or not b.percent for b in buds)


class Controller(QObject):
    """Procura o fone, conecta e reconecta sozinho.

    O HT05 só anuncia BLE por cerca de 60 s depois de conectar o áudio. Por isso o
    scan roda sem parar enquanto o fone estiver desconectado, e a conexão fica aberta.
    """

    state_changed = Signal(object)    # DeviceState
    status_changed = Signal(str)      # texto para a barra de status
    error = Signal(str)
    bluetooth_changed = Signal(bool)    # Bluetooth do sistema ligado ou desligado
    audio_only_changed = Signal(bool)   # áudio conectado, mas o controle BLE não está visível

    def __init__(self, address: str | None = None) -> None:
        super().__init__()
        self.address = address
        self.device: HT05 | None = None
        self._task: asyncio.Task | None = None
        self._stop = False
        self.settings = QSettings("qcyctl", "qcyctl")
        # MAC clássico salvo: permite reabrir o anúncio BLE quando o áudio já está conectado.
        self.classic_mac: str | None = self.settings.value("classic_mac") or None
        self._next_audio_check = 0.0
        self._audio_fails = 0
        self.audio_only = False
        self._audio_seen = False
        self.bluetooth_on = True
        self._probe_due = 0.0
        self._probe_sent: float | None = None
        self._waking = False
        self._lost_at: float | None = None   # instante da queda do BLE

    def start(self) -> None:
        self._task = asyncio.ensure_future(self._loop())

    async def stop(self) -> None:
        self._stop = True
        if self._task:
            self._task.cancel()
        # O elo BLE fica aberto de propósito. O fone só anuncia o controle por cerca de
        # 1 minuto depois de conectar; com o elo aberto, a próxima abertura do app
        # encontra o fone sem reconectar o áudio (ver `_drop_stale_ble`).

    @property
    def connected(self) -> bool:
        return bool(self.device and self.device.is_connected)

    async def _loop(self) -> None:
        while not self._stop:
            if self._waking:  # o adaptador reinicia; o scan volta depois
                await asyncio.sleep(0.5)
                continue
            # Bluetooth do sistema desligado: o app mostra o aviso e não tenta mais nada.
            if not self.connected and await classic.adapter_powered() is False:
                self._bluetooth_off()
                await asyncio.sleep(1.5)
                continue
            if self.connected:
                await self._check_link()
                await self._check_audio()
                await asyncio.sleep(1)
                continue
            # Queda curta do BLE: mantém a tela com o último estado e reconecta em silêncio.
            now = asyncio.get_running_loop().time()
            if self.device is not None and self._lost_at is None:
                self._lost_at = now
                log.info("controle BLE caiu; reconectando")
                if _in_case(self.device.state.battery):
                    # Na caixa, o fone solta o controle, mas o sistema continua a mostrar
                    # o áudio como conectado (o adaptador não percebe a queda). O app
                    # encerra essa conexão para o sistema mostrar o estado real.
                    log.info("fones na caixa; o app encerra a conexão de áudio no sistema")
                    self._lost_at = now - HOLD_S
                    if self.classic_mac:
                        try:
                            await classic.disconnect(self.classic_mac)
                        except Exception as exc:  # noqa: BLE001 - o sistema já pode ter encerrado
                            log.info("falha ao encerrar o áudio: %s", exc)
            holding = self._lost_at is not None and now - self._lost_at < HOLD_S
            # A espera só esconde quedas curtas do controle. Sem o áudio, o fone saiu de
            # verdade (caixa, desligado, desconectado no sistema): a tela mostra isso logo.
            if holding and not await self._audio_up():
                holding = False
            if holding:
                self.status_changed.emit("Reconectando o controle…")
            else:
                self.device = None
                self._lost_at = None
                self.status_changed.emit("Procurando o fone…")
                self.state_changed.emit(DeviceState())
            try:
                # O sistema mantém o elo BLE aberto depois que o app fecha. Esse elo não
                # responde mais, e com ele aberto o fone não anuncia. O app o derruba (o
                # áudio não é afetado); o fone volta a anunciar e o scan o encontra.
                if await self._drop_stale_ble():
                    await asyncio.sleep(0.5)
                found = await find_first_with_adv(timeout=3 if holding else 6)
                self._set_bluetooth(True)  # o scan funcionou
                if found is None:
                    if not holding:
                        await self._check_audio_only()
                    continue
                else:
                    target, adv = found
                    if self.address and target.address.upper() != self.address.upper():
                        continue
                    self.classic_mac = adv.classic_mac
                    self.settings.setValue("classic_mac", adv.classic_mac)
                self._set_audio_only(False)
                if not holding:
                    self.status_changed.emit("Conectando…")
                dev = HT05(target)
                await asyncio.wait_for(dev.connect(timeout=8), 20)
                # Só publica o estado depois da leitura inicial, para a tela não piscar.
                # A queda é tratada por este laço, não pelo estado do fone.
                # Publica uma cópia: o fone marca o próprio estado como desconectado na queda.
                dev.on_change(lambda s: self.state_changed.emit(replace(s)) if s.connected else None)
                self.device = dev
                self._lost_at = None
                self._probe_sent = None
                self._probe_due = time.monotonic() + PROBE_IDLE_S
                self.settings.setValue("ble_mac", target.address)
                self.state_changed.emit(replace(dev.state))
                self.status_changed.emit("Conectado")
                # Ao sair da caixa, o fone chama o computador para o áudio por conta própria.
                # Um Connect() nosso no mesmo instante colide com essa chamada e as duas falham
                # ("Operation already in progress"). Por isso a primeira verificação espera.
                self._audio_fails = 0
                self._audio_seen = False
                self._next_audio_check = asyncio.get_running_loop().time() + AUDIO_GRACE_S
            except asyncio.CancelledError:
                raise
            except BleakBluetoothNotAvailableError:
                # Windows e outros sistemas: o scan é quem informa o Bluetooth desligado.
                self._bluetooth_off()
                await asyncio.sleep(2)
            except Exception as exc:  # noqa: BLE001 - qualquer falha volta ao scan
                log.info("falha de conexão: %s", exc)
                if holding:
                    await asyncio.sleep(1)
                    continue
                self.status_changed.emit("Fone não encontrado. Tire da caixa ou reconecte o áudio.")
                await asyncio.sleep(3)

    def _set_bluetooth(self, on: bool) -> None:
        if on != self.bluetooth_on:
            self.bluetooth_on = on
            log.info("Bluetooth do sistema %s", "ligado" if on else "desligado")
            self.bluetooth_changed.emit(on)

    def _bluetooth_off(self) -> None:
        """Limpa a tela uma vez; depois só espera o Bluetooth voltar."""
        if not self.bluetooth_on:
            return
        self._set_bluetooth(False)
        self.device = None
        self._lost_at = None
        self._set_audio_only(False)
        self.state_changed.emit(DeviceState())
        self.status_changed.emit("Bluetooth desligado")

    def enable_bluetooth(self) -> None:
        """Pedido do usuário: liga o Bluetooth do sistema (Linux)."""
        async def run():
            try:
                await classic.set_powered(True)
            except Exception as exc:  # noqa: BLE001 - bloqueio por rfkill, sem adaptador
                log.info("falha ao ligar o Bluetooth: %s", exc)
                self.error.emit("O sistema não ligou o Bluetooth. Ligue-o nas configurações do sistema.")

        if classic.SUPPORTED:
            asyncio.ensure_future(run())

    async def _drop_stale_ble(self) -> bool:
        """Derruba o elo BLE que o sistema manteve aberto. Retorna True se havia um."""
        mac = self.address or self.settings.value("ble_mac")
        if not mac or not classic.SUPPORTED:
            return False
        try:
            if not (await classic.status(mac)).connected:
                return False
            return await classic.disconnect(mac)
        except Exception:  # noqa: BLE001 - D-Bus indisponível
            return False

    async def _check_link(self) -> None:
        """Detecta o elo BLE mudo: o sistema o mostra conectado, mas o fone não responde.

        O app pede o nome do fone (pedido sem efeito) e espera uma notificação. Sem
        resposta, derruba o elo; o laço principal reconecta.
        """
        dev, now = self.device, time.monotonic()
        if self._probe_sent is not None:
            if dev.last_rx >= self._probe_sent:
                self._probe_sent = None
                self._probe_due = now + PROBE_IDLE_S
                return
            if now - self._probe_sent < PROBE_TIMEOUT_S:
                return
        elif now < self._probe_due:
            return
        else:
            self._probe_sent = now
            try:
                await dev.send_command(p.cmd_request(p.Cmd.PAIR_NAME))
                return
            except Exception as exc:  # noqa: BLE001 - qualquer falha de escrita indica elo morto
                log.info("sonda do controle falhou: %s", exc)
        log.info("o fone não respondeu; o app derruba o elo de controle e reconecta")
        self._probe_sent = None
        self.status_changed.emit("O fone parou de responder. Reconectando…")
        try:
            await asyncio.wait_for(dev.disconnect(), 6)
        except Exception:
            log.debug("falha ao derrubar o elo", exc_info=True)

    async def _audio_up(self) -> bool:
        """Áudio conectado no sistema. Sem suporte ou sem endereço, assume que sim."""
        if not classic.SUPPORTED or not self.classic_mac:
            return True
        try:
            return (await classic.status(self.classic_mac)).connected
        except Exception:  # noqa: BLE001 - D-Bus indisponível
            return True

    def _set_audio_only(self, on: bool) -> None:
        if on != self.audio_only:
            self.audio_only = on
            self.audio_only_changed.emit(on)

    async def _check_audio_only(self) -> None:
        """O HT05 para de anunciar o BLE cerca de 60 s depois de conectar o áudio.

        Nesse estado o app reconhece o fone pelo sistema, mas não alcança o controle.
        O app não reconecta o áudio sozinho: o usuário decide (ver `wake`).
        """
        if self._waking or not classic.SUPPORTED or not self.classic_mac:
            return
        try:
            st = await classic.status(self.classic_mac)
        except Exception:  # noqa: BLE001 - D-Bus indisponível
            return
        self._set_audio_only(st.connected)
        if st.connected:
            self.status_changed.emit("Áudio conectado. Controles em espera.")

    def wake(self) -> None:
        """Pedido do usuário: reconecta o áudio (pausa curta) para o fone anunciar o controle."""
        if not self._waking and self.classic_mac:
            asyncio.ensure_future(self._wake())

    async def _wake(self) -> None:
        self._waking = True
        try:
            self.status_changed.emit("Ativando os controles. O Bluetooth reinicia por alguns segundos…")
            ok = await classic.reconnect(self.classic_mac)
            self._set_audio_only(False)
            if not ok:
                self.error.emit("Não foi possível reconectar o áudio. Desligue e ligue o Bluetooth do sistema.")
        finally:
            self._waking = False

    async def _check_audio(self, force: bool = False) -> None:
        """Garante o áudio clássico conectado (Linux). Repete a cada 15 s, ou 60 s após falha."""
        loop = asyncio.get_running_loop()
        if not classic.SUPPORTED or not self.classic_mac:
            return
        if not force and loop.time() < self._next_audio_check:
            return
        try:
            st = await classic.status(self.classic_mac)
            if st.connected:
                self._audio_fails = 0
                self._audio_seen = True
                self._next_audio_check = loop.time() + 15
                return
            if self._audio_seen and not force:
                # O áudio estava conectado e caiu com o controle ativo: o usuário o
                # desconectou no sistema. O app não reconecta contra a vontade dele.
                self._next_audio_check = loop.time() + 5
                self.status_changed.emit("Controle conectado; áudio desconectado")
                return
            self.status_changed.emit("Conectando o áudio…")
            result = await classic.ensure_connected(self.classic_mac)
        except Exception as exc:  # noqa: BLE001 - D-Bus indisponível, etc.
            result = f"falha ao verificar o áudio: {exc}"
        log.info("áudio: %s", result)
        ok = result == "conectado"
        # Falha: tenta de novo em 3, 6, 12, 24, 48 e depois a cada 60 s.
        self._audio_fails = 0 if ok else self._audio_fails + 1
        self._next_audio_check = loop.time() + (15 if ok else min(60, 1.5 * 2 ** self._audio_fails))
        if not ok and self._audio_fails >= 3 and "br-connection" in result:
            result = "o Bluetooth do sistema não responde. Desligue e ligue o Bluetooth."
        self.status_changed.emit("Conectado (controle e áudio)" if ok else f"Controle conectado; áudio: {result}")

    def connect_audio(self) -> None:
        asyncio.ensure_future(self._check_audio(force=True))

    def run(self, coro_factory) -> None:
        """Executa um comando no fone sem bloquear a interface."""
        async def wrapper():
            if not self.connected:
                self.error.emit("Fone desconectado.")
                return
            try:
                await coro_factory(self.device)
                # Confere logo se o fone ainda responde, para o comando não se perder em silêncio.
                self._probe_due = min(self._probe_due, time.monotonic() + 1.5)
            except (p.ProtocolError, ConnectionError, OSError) as exc:
                self.error.emit(str(exc))
            except Exception as exc:
                log.exception("erro no comando")
                self.error.emit(str(exc))

        asyncio.ensure_future(wrapper())
