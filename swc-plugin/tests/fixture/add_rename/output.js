// Rename feature: $a -> $aRenamed, $b -> $bRenamed, $c -> $cRenamed
angular.module("MyMod").controller("MyCtrl", [
    "$aRenamed",
    "$bRenamed",
    function($a, $b) {}
]);
myMod.service("foo", [
    "$cRenamed",
    "$aRenamed",
    function($c, $a) {}
]);
myMod.factory("foo", [
    "$bRenamed",
    "$cRenamed",
    function($b, $c) {}
]);
