// ngInject detection: comment and directive prologue forms

// @ngInject comment on function declaration
// @ngInject
function foo($scope, $timeout) {}

// @ngInject comment on var declaration
// @ngInject
var bar = function($scope) {};

// @ngInject comment on arrow function var
// @ngInject
var baz = ($a, $b) => {};

// ngInject directive prologue in function
function Foo2($scope) {
    "ngInject";
}

// ngInject directive prologue in arrow
var foos3 = ($scope) => {
    "ngInject";
};

// @ngNoInject suppression
// @ngInject
function suppressed() {}
myMod.controller("suppressed", /*@ngNoInject*/ function($scope) {});
