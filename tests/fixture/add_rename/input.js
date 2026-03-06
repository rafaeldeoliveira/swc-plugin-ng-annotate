// Rename feature: $a -> $aRenamed, $b -> $bRenamed, $c -> $cRenamed

angular.module("MyMod").controller("MyCtrl", function($a, $b) {});
myMod.service("foo", function($c, $a) {});
myMod.factory("foo", function($b, $c) {});
