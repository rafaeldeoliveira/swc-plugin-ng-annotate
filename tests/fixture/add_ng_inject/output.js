// ngInject detection: comment and directive prologue forms
// @ngInject comment on function declaration
// @ngInject
function foo($scope, $timeout) {}
foo.$inject = [
    "$scope",
    "$timeout"
];
// @ngInject comment on var declaration
// @ngInject
var bar = function($scope) {};
bar.$inject = [
    "$scope"
];
// @ngInject comment on arrow function var
// @ngInject
var baz = ($a, $b)=>{};
baz.$inject = [
    "$a",
    "$b"
];
// ngInject directive prologue in function
function Foo2($scope) {
    "ngInject";
}
Foo2.$inject = [
    "$scope"
];
// ngInject directive prologue in arrow
var foos3 = ($scope)=>{
    "ngInject";
};
foos3.$inject = [
    "$scope"
];
// @ngNoInject suppression
// @ngInject
function suppressed() {}
myMod.controller("suppressed", /*@ngNoInject*/ function($scope) {});
