import QtQuick
import Qcy

// Slider de passos inteiros com marcas. Arraste, clique ou use as setas.
Item {
    id: root
    property int from: 1
    property int to: 6
    property int value: 1
    signal moved(int value)
    implicitHeight: 36
    implicitWidth: 260
    activeFocusOnTab: true
    readonly property int steps: to - from
    readonly property real usable: width - 20
    function valueAt(x) { return Math.round(Math.max(0, Math.min(1, (x - 10) / usable)) * steps) + from }

    Keys.onLeftPressed: if (value > from) moved(value - 1)
    Keys.onRightPressed: if (value < to) moved(value + 1)

    Rectangle {
        id: track
        x: 10; width: root.usable; height: 4; radius: 2
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.surfaceHi
        Rectangle {
            height: parent.height; radius: 2
            width: knob.x + knob.width / 2 - 10
            color: Theme.accent
        }
    }
    Repeater {
        model: root.steps + 1
        Rectangle {
            required property int index
            width: 2; height: 8; radius: 1
            x: 10 + index * root.usable / root.steps - 1
            anchors.verticalCenter: parent.verticalCenter
            color: index + root.from <= root.value ? Theme.accent : Theme.text3
            opacity: 0.6
        }
    }
    Rectangle {
        id: knob
        width: 20; height: 20; radius: 10
        anchors.verticalCenter: parent.verticalCenter
        x: (root.value - root.from) / root.steps * root.usable
        color: Theme.text
        border.width: 4
        border.color: Theme.accent
        scale: ma.pressed ? 1.12 : 1
        Behavior on scale { NumberAnimation { duration: Theme.instant } }
        Behavior on x {
            enabled: !Theme.reduce && !ma.pressed
            SpringAnimation { spring: Theme.snappySpring; damping: Theme.snappyDamping; epsilon: 0.25 }
        }
        FocusRing { show: root.activeFocus && Theme.byKeyboard(root.focusReason); r: 10 }
    }
    MouseArea {
        id: ma
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onPressed: (m) => { root.forceActiveFocus(Qt.MouseFocusReason); update(m.x) }
        onPositionChanged: (m) => update(m.x)
        function update(x) { var v = root.valueAt(x); if (v !== root.value) root.moved(v) }
    }
}
