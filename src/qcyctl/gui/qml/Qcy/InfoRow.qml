import QtQuick
import Qcy

Item {
    property string label: ""
    property string value: ""
    width: parent ? parent.width : 300
    height: 30
    visible: value !== ""
    Text {
        anchors.verticalCenter: parent.verticalCenter
        text: parent.label
        font.family: Theme.family; font.pixelSize: Theme.sizeBody
        color: Theme.text2
    }
    Text {
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: parent.value
        font.family: Theme.family; font.pixelSize: Theme.sizeBody
        font.features: { "tnum": 1 }
        color: Theme.text
    }
}
