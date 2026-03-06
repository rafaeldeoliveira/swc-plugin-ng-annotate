// Reference following: function defined elsewhere, referenced in Angular call
function MyCtrl($scope, $timeout) {}
MyCtrl.$inject = [
    "$scope",
    "$timeout"
];
angular.module("MyMod").controller("bar", MyCtrl);
// var reference
var MyFactory = function($a, $b) {};
MyFactory.$inject = [
    "$a",
    "$b"
];
angular.module("MyMod").factory("foo", MyFactory);
// ngInject comment on reference
function MyCtrl2($scope) {}
MyCtrl2.$inject = [
    "$scope"
];
angular.module("MyMod").controller("bar2", MyCtrl2);
